//! The tests' SSH keys, moved from the SFTP fake: two host keys , public in this repository.
//! What a reader gets wrong: they protect nothing; never use one anywhere real.

pub const HOST_A: &str = "\
-----BEGIN OPENSSH PRIVATE KEY-----\n\
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW\n\
QyNTUxOQAAACA1mSKTen+rmgGzlx8OhCDlXk6YY1x8Hs4RukwAMlhqMQAAAJgzSmV5M0pl\n\
eQAAAAtzc2gtZWQyNTUxOQAAACA1mSKTen+rmgGzlx8OhCDlXk6YY1x8Hs4RukwAMlhqMQ\n\
AAAEA8EJhCEEy7ED/8EdYwUycmH1dpAAu8N8fDGQV8zHTILjWZIpN6f6uaAbOXHw6EIOVe\n\
TphjXHwezhG6TAAyWGoxAAAAEms3IHNmdHAgdGVzdCBob3N0QQECAw==\n\
-----END OPENSSH PRIVATE KEY-----\n\
";

pub const HOST_B: &str = "\
-----BEGIN OPENSSH PRIVATE KEY-----\n\
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW\n\
QyNTUxOQAAACCporp3+gbcXx8IkVZ7Z3Dl8Ge+c8HUtJzeRaQGl1KIMgAAAJj0TUVV9E1F\n\
VQAAAAtzc2gtZWQyNTUxOQAAACCporp3+gbcXx8IkVZ7Z3Dl8Ge+c8HUtJzeRaQGl1KIMg\n\
AAAECZ9wZ7MO740tVCLtsf/8iJsXPRCLEP//Dlk2cnZTJDJqmiunf6BtxfHwiRVntncOXw\n\
Z75zwdS0nN5FpAaXUogyAAAAEms3IHNmdHAgdGVzdCBob3N0QgECAw==\n\
-----END OPENSSH PRIVATE KEY-----\n\
";
