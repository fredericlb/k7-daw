//! An in-process SSH server that runs `git upload-pack` and `git receive-pack` over a folder of bare repositories.
//! What a reader gets wrong: it counts the authentication attempts it saw, which is how a test proves that a
//! changed host key reached no credential; the host key it shows can be switched between two.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::keys::{HashAlg, decode_secret_key};
use russh::server::{Auth, Msg, Server as _, Session, VerifiedPublicKey};
use russh::{Channel, ChannelId};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use super::ssh_keys as keys;

pub const USER: &str = "git";

pub struct Sshd {
    pub addr: SocketAddr,
    pub attempts: Arc<AtomicUsize>,
    pub commands: Arc<Mutex<Vec<String>>>,
    /// When set, an exec is accepted and then nothing is ever written.
    pub stall: Arc<std::sync::atomic::AtomicBool>,
    /// What `git-lfs-authenticate` answers: the endpoint and the `Authorization` it grants.
    pub lfs_grant: Arc<Mutex<Option<(String, String)>>>,
    /// When set, `git-lfs-authenticate` prints this to stderr and exits 1, as a host refusing the repository does.
    pub lfs_error: Arc<Mutex<Option<String>>>,
    /// The public keys (base64 bodies) the server signs in; none until a test authorizes the device's.
    authorized: Arc<Mutex<Vec<String>>>,
    host_key: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Sshd {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn config(key: &str) -> Arc<russh::server::Config> {
    Arc::new(russh::server::Config {
        keys: vec![decode_secret_key(key, None).unwrap()],
        auth_rejection_time: Duration::ZERO,
        auth_rejection_time_initial: Some(Duration::ZERO),
        ..Default::default()
    })
}

pub fn fingerprint(key: &str) -> String {
    decode_secret_key(key, None)
        .unwrap()
        .public_key()
        .fingerprint(HashAlg::Sha256)
        .to_string()
}

impl Sshd {
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    pub fn url(&self, repo: &str) -> String {
        format!("ssh://{USER}@127.0.0.1:{}/{repo}", self.addr.port())
    }

    /// Lets a public key (an `authorized_keys` line) sign in.
    pub fn authorize(&self, line: &str) {
        let body = line.split_whitespace().nth(1).expect("a public key line");
        self.authorized.lock().unwrap().push(body.to_string());
    }

    /// Shows host key B to the connections made from now on.
    pub fn change_host_key(&self) {
        self.host_key.store(1, Ordering::SeqCst);
    }

    pub async fn start(root: PathBuf) -> Sshd {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        let attempts = Arc::new(AtomicUsize::new(0));
        let commands = Arc::new(Mutex::new(Vec::new()));
        let host_key = Arc::new(AtomicUsize::new(0));
        let configs = [config(keys::HOST_A), config(keys::HOST_B)];
        let lfs_grant: Arc<Mutex<Option<(String, String)>>> = Arc::default();
        let authorized: Arc<Mutex<Vec<String>>> = Arc::default();
        let lfs_error: Arc<Mutex<Option<String>>> = Arc::default();
        let refusal = lfs_error.clone();
        let allowed = authorized.clone();
        let grant = lfs_grant.clone();
        let stall = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (a, c, h, st) = (
            attempts.clone(),
            commands.clone(),
            host_key.clone(),
            stall.clone(),
        );
        let task = tokio::spawn(async move {
            while let Ok((s, _)) = l.accept().await {
                let cfg = configs[h.load(Ordering::SeqCst)].clone();
                let mut machine = Machine {
                    root: root.clone(),
                    attempts: a.clone(),
                    commands: c.clone(),
                    stall: st.clone(),
                    grant: grant.clone(),
                    refusal: refusal.clone(),
                    authorized: allowed.clone(),
                };
                tokio::spawn(async move {
                    let handler = machine.new_client(None);
                    if let Ok(running) = russh::server::run_stream(cfg, s, handler).await {
                        let _ = running.await;
                    }
                });
            }
        });
        Sshd {
            addr,
            attempts,
            commands,
            stall,
            lfs_grant,
            lfs_error,
            authorized,
            host_key,
            task,
        }
    }
}

#[derive(Clone)]
struct Machine {
    root: PathBuf,
    attempts: Arc<AtomicUsize>,
    commands: Arc<Mutex<Vec<String>>>,
    stall: Arc<std::sync::atomic::AtomicBool>,
    grant: Arc<Mutex<Option<(String, String)>>>,
    refusal: Arc<Mutex<Option<String>>>,
    authorized: Arc<Mutex<Vec<String>>>,
}

struct Conn {
    m: Machine,
    held: Vec<Channel<Msg>>,
    channels: std::collections::HashMap<ChannelId, Channel<Msg>>,
}

impl russh::server::Server for Machine {
    type Handler = Conn;
    fn new_client(&mut self, _: Option<SocketAddr>) -> Conn {
        Conn {
            m: self.clone(),
            held: Vec::new(),
            channels: Default::default(),
        }
    }
}

/// `git-upload-pack '/repo.git'` taken apart: the program and the path.
fn split(cmd: &str) -> Option<(&'static str, String)> {
    let (prog, rest) = cmd.split_once(' ')?;
    let which = match prog {
        "git-upload-pack" => "upload-pack",
        "git-receive-pack" => "receive-pack",
        _ => return None,
    };
    let path = rest
        .trim()
        .strip_prefix('\'')?
        .strip_suffix('\'')?
        .replace("'\\''", "'");
    Some((which, path))
}

impl russh::server::Handler for Conn {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        user: &str,
        key: VerifiedPublicKey<'_>,
    ) -> Result<Auth, Self::Error> {
        self.m.attempts.fetch_add(1, Ordering::SeqCst);
        let known =
            self.m.authorized.lock().unwrap().iter().any(|b| {
                russh::keys::parse_public_key_base64(b).is_ok_and(|k| k == *key.public_key)
            });
        Ok(if user == USER && known {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: russh::server::ChannelOpenHandle,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let cmd = String::from_utf8_lossy(data).to_string();
        self.m.commands.lock().unwrap().push(cmd.clone());
        if cmd.starts_with("git-lfs-authenticate ") {
            // GitHub does not unquote this path: a quoted one is a repository that does not exist.
            if cmd.contains('\'') {
                let Some(channel) = self.channels.remove(&id) else {
                    return Ok(());
                };
                session.channel_success(id)?;
                let handle = session.handle();
                tokio::spawn(async move {
                    let _ = channel
                        .make_writer_ext(Some(1))
                        .write_all(b"ERROR: invalid repository\n")
                        .await;
                    let _ = channel.exit_status(1).await;
                    let _ = channel.eof().await;
                    let _ = handle.close(id).await;
                });
                return Ok(());
            }
            let Some(channel) = self.channels.remove(&id) else {
                return Ok(());
            };
            session.channel_success(id)?;
            let grant = self.m.grant.lock().unwrap().clone();
            let handle = session.handle();
            let refusal = self.m.refusal.lock().unwrap().clone();
            tokio::spawn(async move {
                if let Some(text) = refusal {
                    let _ = channel
                        .make_writer_ext(Some(1))
                        .write_all(text.as_bytes())
                        .await;
                    let _ = channel.exit_status(1).await;
                    let _ = channel.eof().await;
                    let _ = handle.close(id).await;
                    return;
                }
                let mut s = channel.into_stream();
                if let Some((href, auth)) = grant {
                    let text = serde_json::json!({ "href": href, "header": { "Authorization": auth }, "expires_in": 3600 }).to_string();
                    let _ = s.write_all(text.as_bytes()).await;
                }
                let _ = s.shutdown().await;
                let _ = handle.close(id).await;
            });
            return Ok(());
        }
        let (Some(channel), Some((which, path))) = (self.channels.remove(&id), split(&cmd)) else {
            session.channel_failure(id)?;
            session.close(id)?;
            return Ok(());
        };
        session.channel_success(id)?;
        if self.m.stall.load(Ordering::SeqCst) {
            self.held.push(channel);
            return Ok(());
        }
        let repo = self.m.root.join(path.trim_start_matches('/'));
        let handle = session.handle();
        tokio::spawn(async move {
            let mut child = Command::new("git")
                .arg(which)
                .arg(&repo)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .expect("git upload-pack");
            let (mut rd, mut wr) = tokio::io::split(channel.into_stream());
            let mut stdin = child.stdin.take().unwrap();
            let mut stdout = child.stdout.take().unwrap();
            let feed = tokio::spawn(async move {
                let mut buf = vec![0u8; 32 * 1024];
                while let Ok(n) = rd.read(&mut buf).await {
                    if n == 0 || stdin.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            });
            let mut buf = vec![0u8; 32 * 1024];
            while let Ok(n) = stdout.read(&mut buf).await {
                if n == 0 || wr.write_all(&buf[..n]).await.is_err() {
                    break;
                }
            }
            let status = child.wait().await.ok().and_then(|s| s.code()).unwrap_or(1);
            let _ = wr.shutdown().await;
            let _ = handle.exit_status_request(id, status as u32).await;
            let _ = handle.close(id).await;
            feed.abort();
        });
        Ok(())
    }
}
