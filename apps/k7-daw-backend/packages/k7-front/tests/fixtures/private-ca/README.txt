A private CA (ca.pem, its key thrown away) and a loopback server's certificate and key it signed, valid for a
century: tests/trust.rs serves TLS with them and checks a client trusts the server only with ca.pem added.
