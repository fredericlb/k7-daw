//! ssh.rs — git over SSH on russh: `git-upload-pack '<path>'` or `git-receive-pack` on an exec channel.
//! What a reader gets wrong: the server's key is judged during the key exchange, BEFORE any credential is offered;
//! an unknown or changed key stops the call with a typed error and the secret store is never read.

use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use git2::transport::{Service, SmartSubtransport, SmartSubtransportStream};
use russh::Disconnect;
use russh::client::{self, Handle};
use russh::keys::{HashAlg, PrivateKey, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{Net, fail, with_op};
use crate::Error;
use crate::auth::{KnownHosts, device_key, old_key};

const READY: Duration = Duration::from_secs(20);

/// An ssh address taken apart; `path` is what the server's git is asked to serve.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Address {
    pub user: Option<String>,
    pub host: String,
    pub port: u16,
    pub explicit_port: bool,
    pub path: String,
}

/// `ssh://[user@]host[:port]/path` or `[user@]host:path`.
pub(crate) fn parse(url: &str) -> Result<Address, Error> {
    let bad = |m: &str| Error::BadUrl(m.to_string());
    let (authority, path, port) = if let Some(rest) = url.strip_prefix("ssh://") {
        let (auth, path) = rest
            .split_once('/')
            .ok_or_else(|| bad("no path after the host"))?;
        let (userhost, port) = match auth.rsplit_once(':') {
            Some((uh, p)) if p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty() => (
                uh,
                Some(
                    p.parse::<u16>()
                        .map_err(|_| bad("the port is not a number"))?,
                ),
            ),
            _ => (auth, None),
        };
        (userhost, format!("/{path}"), port)
    } else {
        let (auth, path) = url
            .split_once(':')
            .ok_or_else(|| bad("not an ssh address"))?;
        (auth, path.to_string(), None)
    };
    if path.trim_matches('/').is_empty() {
        return Err(bad("no repository path"));
    }
    let (user, host) = match authority.rsplit_once('@') {
        Some((u, _)) if u.contains(':') => {
            return Err(bad("remove the password from the address; k7 asks for it"));
        }
        Some((u, h)) => (Some(u.to_string()), h),
        None => (None, authority),
    };
    if host.is_empty() {
        return Err(bad("no host"));
    }
    Ok(Address {
        user,
        host: host.trim_matches(['[', ']']).to_string(),
        port: port.unwrap_or(22),
        explicit_port: port.is_some(),
        path,
    })
}

/// `'path'` for a shell: single quotes, an inner quote closed and reopened.
pub(crate) fn quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\\''"))
}

struct Pin {
    hosts: Arc<KnownHosts>,
    host: String,
    port: u16,
    verdict: Arc<Mutex<Option<Error>>>,
}

impl client::Handler for Pin {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let PublicKeyOrCertificate::PublicKey { key, .. } = key else {
            return Ok(false);
        };
        let got = key.fingerprint(HashAlg::Sha256).to_string();
        let verdict = match self.hosts.get(&self.host, self.port) {
            Ok(Some(listed)) if listed == got => return Ok(true),
            Ok(Some(expected)) => Error::HostKeyChanged { expected, got },
            Ok(None) => Error::UnknownHost { fingerprint: got },
            Err(e) => Error::Io(format!("the trusted servers' list: {e}")),
        };
        *self.verdict.lock().unwrap_or_else(|p| p.into_inner()) = Some(verdict);
        Ok(false)
    }
}

/// A signed-in exec channel; the session handle rides along so the connection lives as long as the stream.
pub(crate) struct Conn {
    _session: Handle<Pin>,
    io: tokio::sync::Mutex<russh::ChannelStream<client::Msg>>,
    net: Arc<Net>,
}

pub(crate) struct SshSub {
    cur: Mutex<Option<Arc<Conn>>>,
}

impl SshSub {
    pub fn new() -> SshSub {
        SshSub {
            cur: Mutex::new(None),
        }
    }
}

impl SmartSubtransport for SshSub {
    fn action(
        &self,
        url: &str,
        service: Service,
    ) -> Result<Box<dyn SmartSubtransportStream>, git2::Error> {
        let mut cur = self.cur.lock().unwrap_or_else(|p| p.into_inner());
        if matches!(service, Service::UploadPack | Service::ReceivePack) {
            return match &*cur {
                Some(c) => Ok(Box::new(Stream(c.clone()))),
                None => Err(git2::Error::from_str("the ssh session is gone")),
            };
        }
        let (net, user) = with_op(|op| (op.net.clone(), op.user.clone()))?;
        let command = match service {
            Service::UploadPackLs => "git-upload-pack",
            _ => "git-receive-pack",
        };
        let addr = parse(url).map_err(fail)?;
        let cmd = format!("{command} {}", quote(&addr.path));
        let conn = open(&net, &addr, user, cmd).map_err(fail)?;
        let conn = Arc::new(conn);
        *cur = Some(conn.clone());
        Ok(Box::new(Stream(conn)))
    }

    fn close(&self) -> Result<(), git2::Error> {
        if let Some(c) = self.cur.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let net = c.net.clone();
            net.handle.block_on(async {
                let _ = c.io.lock().await.shutdown().await;
                let _ = c
                    ._session
                    .disconnect(Disconnect::ByApplication, "", "en")
                    .await;
            });
        }
        Ok(())
    }
}

pub(crate) fn open(
    net: &Arc<Net>,
    addr: &Address,
    user: Option<String>,
    cmd: String,
) -> Result<Conn, Error> {
    let (ssh, channel) = start(net, addr, user, cmd)?;
    Ok(Conn {
        _session: ssh,
        io: tokio::sync::Mutex::new(channel.into_stream()),
        net: net.clone(),
    })
}

/// What a command printed and how it ended: the host's own words are in `stderr`.
pub(crate) struct Reply {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: Option<u32>,
}

/// Runs `cmd` to its end and keeps both streams and the exit status.
pub(crate) fn run(
    net: &Arc<Net>,
    addr: &Address,
    user: Option<String>,
    cmd: String,
) -> Result<Reply, Error> {
    let (_ssh, mut channel) = start(net, addr, user, cmd)?;
    let mut reply = Reply {
        stdout: Vec::new(),
        stderr: Vec::new(),
        status: None,
    };
    let drain = async {
        while let Some(m) = channel.wait().await {
            match m {
                russh::ChannelMsg::Data { data } => reply.stdout.extend_from_slice(&data),
                russh::ChannelMsg::ExtendedData { data, .. } => {
                    reply.stderr.extend_from_slice(&data)
                }
                russh::ChannelMsg::ExitStatus { exit_status } => reply.status = Some(exit_status),
                _ => {}
            }
        }
    };
    net.handle
        .block_on(async { tokio::time::timeout(net.idle, drain).await })
        .map_err(|_| Error::Network("the server stopped answering".into()))?;
    Ok(reply)
}

fn start(
    net: &Arc<Net>,
    addr: &Address,
    user: Option<String>,
    cmd: String,
) -> Result<(Handle<Pin>, russh::Channel<client::Msg>), Error> {
    let user = user
        .or_else(|| addr.user.clone())
        .unwrap_or_else(|| "git".to_string());
    let verdict = Arc::new(Mutex::new(None));
    let pin = Pin {
        hosts: net.hosts.clone(),
        host: addr.host.clone(),
        port: addr.port,
        verdict: verdict.clone(),
    };
    let work = async {
        let config = Arc::new(client::Config::default());
        let mut ssh = client::connect(config, (addr.host.as_str(), addr.port), pin)
            .await
            .map_err(|e| key_verdict(&verdict, e))?;
        sign_in(
            &mut ssh,
            net,
            &user,
            &format!("{}:{}", addr.host, addr.port),
        )
        .await?;
        let channel = ssh
            .channel_open_session()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;
        channel
            .exec(true, cmd)
            .await
            .map_err(|e| Error::Network(e.to_string()))?;
        Ok::<_, Error>((ssh, channel))
    };
    net.handle
        .block_on(async { tokio::time::timeout(READY, work).await })
        .map_err(|_| Error::Network("the server did not answer in time".into()))?
}

/// A refused key is told apart from a refused connection by what the handler saw.
fn key_verdict(verdict: &Mutex<Option<Error>>, e: russh::Error) -> Error {
    verdict
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take()
        .unwrap_or_else(|| Error::Network(format!("the server cannot be reached: {e}")))
}

async fn offer(ssh: &mut Handle<Pin>, user: &str, key: PrivateKey) -> Result<bool, Error> {
    let hash = if key.algorithm().is_rsa() {
        ssh.best_supported_rsa_hash()
            .await
            .map_err(|e| Error::Network(e.to_string()))?
            .unwrap_or(Some(HashAlg::Sha256))
    } else {
        None
    };
    Ok(ssh
        .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
        .await
        .map_err(|e| Error::Network(e.to_string()))?
        .success())
}

/// The device key, then the Mac's old key once when the host refused it; which one worked is noted per host.
async fn sign_in(ssh: &mut Handle<Pin>, net: &Net, user: &str, at: &str) -> Result<(), Error> {
    let key = device_key(&*net.secrets)?;
    let mut old_worked = false;
    let mut ok = offer(ssh, user, key).await?;
    if !ok && let Some(old) = old_key(&*net.secrets)? {
        ok = offer(ssh, user, old).await?;
        old_worked = ok;
    }
    if ok {
        let mut hosts = net.old_key_hosts.lock().unwrap_or_else(|p| p.into_inner());
        if old_worked {
            hosts.insert(at.to_string());
        } else {
            hosts.remove(at);
        }
        return Ok(());
    }
    Err(Error::Auth(
        "the server refused the device's key for this user: open Settings → Services to add this device's key, or add its public key to the host"
            .into(),
    ))
}

struct Stream(Arc<Conn>);

fn stalled() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "the server stopped answering")
}

impl Read for Stream {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let c = &self.0;
        c.net
            .handle
            .block_on(async { tokio::time::timeout(c.net.idle, c.io.lock().await.read(out)).await })
            .map_err(|_| stalled())?
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let c = &self.0;
        c.net
            .handle
            .block_on(async {
                tokio::time::timeout(c.net.idle, c.io.lock().await.write(buf)).await
            })
            .map_err(|_| stalled())?
    }

    fn flush(&mut self) -> io::Result<()> {
        let c = &self.0;
        c.net
            .handle
            .block_on(async { tokio::time::timeout(c.net.idle, c.io.lock().await.flush()).await })
            .map_err(|_| stalled())?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_address_forms_are_taken_apart() {
        let a = parse("ssh://ann@nas.local:2222/srv/songs.git").unwrap();
        assert_eq!(
            (a.user.as_deref(), a.host.as_str(), a.port, a.path.as_str()),
            (Some("ann"), "nas.local", 2222, "/srv/songs.git")
        );
        let b = parse("git@github.com:ann/songs.git").unwrap();
        assert_eq!(
            (b.user.as_deref(), b.host.as_str(), b.port, b.path.as_str()),
            (Some("git"), "github.com", 22, "ann/songs.git")
        );
        assert!(!b.explicit_port);
        let c = parse("ssh://host/~/songs.git").unwrap();
        assert_eq!((c.user, c.port), (None, 22));
    }

    #[test]
    fn an_address_with_a_password_or_no_path_is_refused() {
        assert!(matches!(
            parse("ssh://ann:pw@host/r.git"),
            Err(Error::BadUrl(_))
        ));
        assert!(matches!(parse("ssh://host"), Err(Error::BadUrl(_))));
        assert!(matches!(parse("ssh://host/"), Err(Error::BadUrl(_))));
    }

    #[test]
    fn a_path_is_quoted_for_the_servers_shell() {
        assert_eq!(quote("/srv/a b.git"), "'/srv/a b.git'");
        assert_eq!(quote("/it's.git"), "'/it'\\''s.git'");
    }
}
