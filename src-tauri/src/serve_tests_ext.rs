#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_secret_short_and_long() {
        assert_eq!(mask_secret(""), "");
        assert_eq!(mask_secret("ab"), "••••");
        assert_eq!(mask_secret("super-secret-token"), "••••oken");
        assert_eq!(mask_secret("abcd"), "••••");
        assert_eq!(mask_secret("abcde"), "••••bcde");
    }

    #[test]
    fn secret_last4_works() {
        assert_eq!(secret_last4(""), "");
        assert_eq!(secret_last4("xy"), "xy");
        assert_eq!(secret_last4("super-secret-token"), "oken");
    }

    #[test]
    fn generate_secret_is_long_url_safe() {
        let s = generate_serve_secret();
        assert!(s.len() >= 32, "len {}", s.len());
        assert!(!s.contains('+') && !s.contains('/') && !s.contains('='));
    }

    #[test]
    fn normalize_bind_default_and_valid() {
        assert_eq!(normalize_bind(None).unwrap(), DEFAULT_SERVE_BIND);
        assert_eq!(normalize_bind(Some("")).unwrap(), DEFAULT_SERVE_BIND);
        assert_eq!(
            normalize_bind(Some("127.0.0.1:3000")).unwrap(),
            "127.0.0.1:3000"
        );
        assert!(normalize_bind(Some("http://evil")).is_err());
        assert!(normalize_bind(Some("not-a-bind")).is_err());
    }

    #[test]
    fn build_connection_url_shape() {
        let url = build_connection_url("127.0.0.1:2419", "tokensecret99");
        assert_eq!(
            url,
            "ws://127.0.0.1:2419/ws?server-key=tokensecret99"
        );
        let masked = build_connection_url_masked("127.0.0.1:2419", "tokensecret99");
        assert!(masked.contains("••••et99"));
        assert!(!masked.contains("tokensecret99"));
    }

    #[test]
    fn build_connection_cli_template_and_mask() {
        let cli = build_connection_cli("127.0.0.1:2419", "tokensecret99");
        assert_eq!(
            cli,
            "GROK_SERVE_SECRET=tokensecret99 grok --remote ws://127.0.0.1:2419/ws"
        );
        let masked = build_connection_cli_masked("127.0.0.1:2419", "tokensecret99");
        assert!(masked.contains("••••et99"));
        assert!(!masked.contains("tokensecret99"));
        assert!(masked.starts_with("GROK_SERVE_SECRET="));
        assert!(!masked.contains("--secret"));
        assert!(!cli.contains("--secret"));
        assert_eq!(build_remote_ws_base("127.0.0.1:2419"), "ws://127.0.0.1:2419/ws");
    }

    #[test]
    fn normalize_remote_url_accepts_ws_and_http() {
        assert_eq!(normalize_remote_url(None).unwrap(), None);
        assert_eq!(normalize_remote_url(Some("")).unwrap(), None);
        assert_eq!(
            normalize_remote_url(Some("  ws://upstream.example:9000/agent  "))
                .unwrap()
                .as_deref(),
            Some("ws://upstream.example:9000/agent")
        );
        assert_eq!(
            normalize_remote_url(Some("wss://edge.example/ws"))
                .unwrap()
                .as_deref(),
            Some("wss://edge.example/ws")
        );
        assert_eq!(
            normalize_remote_url(Some("https://edge.example/ws"))
                .unwrap()
                .as_deref(),
            Some("wss://edge.example/ws")
        );
        assert_eq!(
            normalize_remote_url(Some("http://127.0.0.1:3000/ws"))
                .unwrap()
                .as_deref(),
            Some("ws://127.0.0.1:3000/ws")
        );
        assert!(normalize_remote_url(Some("ftp://x")).is_err());
        assert!(normalize_remote_url(Some("not-a-url")).is_err());
        assert!(normalize_remote_url(Some("ws://x y")).is_err());
        assert!(normalize_remote_url(Some("ws://")).is_err());
        assert!(
            normalize_remote_url(Some("ws://h/ws?server-key=sekrit")).is_err(),
            "must reject secret in remote query"
        );
        assert!(normalize_remote_url(Some("ws://h/ws?token=abc")).is_err());
    }

    #[test]
    fn parse_serve_help_requires_bind_and_secret() {
        let ok_help = r#"
Run the agent as a WebSocket server
Options:
  --bind <BIND>      Address for the server to listen on
  --secret <SECRET>  Secret token for client authentication
  --remote <REMOTE>  Remote agent URL for proxy mode
"#;
        assert!(parse_serve_help_supports(ok_help, "", true));
        assert!(parse_serve_help_supports_remote(ok_help, "", true));
        assert!(!parse_serve_help_supports(ok_help, "", false));
        assert!(!parse_serve_help_supports("unknown command", "", true));
        assert!(!parse_serve_help_supports("--bind only", "", true));
        assert!(!parse_serve_help_supports_remote(
            "--bind and --secret only, no proxy",
            "",
            true
        ));
    }

    #[test]
    fn derive_state_matrix() {
        let (s, m) = derive_serve_state(false, false, false, false, None);
        assert_eq!(s, "error");
        assert!(m.unwrap().contains("not found"));

        let (s, m) = derive_serve_state(true, false, false, false, None);
        assert_eq!(s, "unsupported");
        assert!(m.unwrap().contains("agent serve"));

        let (s, _) = derive_serve_state(true, true, true, false, None);
        assert_eq!(s, "running");

        let (s, _) = derive_serve_state(true, true, false, true, None);
        assert_eq!(s, "running");

        let (s, _) = derive_serve_state(true, true, false, false, None);
        assert_eq!(s, "stopped");
    }

    #[test]
    fn status_dto_serde_omits_full_secret_fields_when_none() {
        let dto = ServeStatusDto {
            state: "stopped".into(),
            bind: DEFAULT_SERVE_BIND.into(),
            remote: None,
            secret_masked: None,
            secret_last4: None,
            connection_url: None,
            connection_cli: None,
            connection_cli_masked: None,
            pid: None,
            tracked_pid: None,
            port_open: false,
            cli_found: true,
            cli_supports_serve: true,
            cli_supports_remote: true,
            non_loopback: false,
            exposure_warning: None,
            message: None,
        };
        let v = serde_json::to_value(&dto).unwrap();
        assert!(v.get("secretMasked").is_none() || v.get("secretMasked").unwrap().is_null());
        assert!(v.get("connectionUrl").is_none() || v.get("connectionUrl").unwrap().is_null());
        assert!(v.get("connectionCli").is_none() || v.get("connectionCli").unwrap().is_null());
        // Never a raw "secret" field.
        assert!(v.get("secret").is_none());
    }

    #[test]
    fn normalize_probe_addr_accepts_host_port_only() {
        assert_eq!(
            normalize_probe_addr("127.0.0.1:2419").unwrap(),
            "127.0.0.1:2419"
        );
        assert!(normalize_probe_addr("").is_err());
        assert!(normalize_probe_addr("ws://127.0.0.1:2419/ws?server-key=x").is_err());
        assert!(normalize_probe_addr("127.0.0.1:2419/ws").is_err());
        assert!(normalize_probe_addr("127.0.0.1:2419?server-key=abc").is_err());
        assert!(normalize_probe_addr("host with space:1").is_err());
    }

    #[test]
    fn normalize_bind_detects_non_loopback() {
        assert!(!is_non_loopback_bind("127.0.0.1:2419"));
        assert!(!is_non_loopback_bind("localhost:2419"));
        assert!(!is_non_loopback_bind("[::1]:2419"));

        assert!(is_non_loopback_bind("0.0.0.0:2419"));
        assert!(is_non_loopback_bind("192.168.1.1:2419"));
        assert!(is_non_loopback_bind("10.0.0.5:2419"));

        // normalize_bind succeeds on valid non-loopback addresses while triggering security logging
        let res = normalize_bind(Some("0.0.0.0:2419"));
        assert_eq!(res.unwrap(), "0.0.0.0:2419");
    }

    #[test]
    fn spawn_serve_process_passes_secret_via_env() {
        let cmd = build_serve_command(
            Path::new("/bin/grok"),
            "127.0.0.1:2419",
            "sekrit-token-123",
            Some("ws://upstream:9000/ws"),
        );
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().to_string()).collect();
        // argv must NOT contain --secret or the token
        assert!(!args.contains(&"--secret".to_string()));
        assert!(!args.contains(&"sekrit-token-123".to_string()));
        assert!(args.contains(&"agent".to_string()));
        assert!(args.contains(&"serve".to_string()));
        assert!(args.contains(&"--bind".to_string()));
        assert!(args.contains(&"127.0.0.1:2419".to_string()));
        assert!(args.contains(&"--remote".to_string()));
        assert!(args.contains(&"ws://upstream:9000/ws".to_string()));

        // env must contain GROK_SERVE_SECRET=sekrit-token-123
        let envs: std::collections::HashMap<String, Option<String>> = cmd
            .get_envs()
            .map(|(k, v)| (k.to_string_lossy().to_string(), v.map(|v| v.to_string_lossy().to_string())))
            .collect();
        assert_eq!(
            envs.get("GROK_SERVE_SECRET"),
            Some(&Some("sekrit-token-123".to_string()))
        );
        assert_eq!(
            envs.get("GROK_AGENT_SECRET"),
            Some(&Some("sekrit-token-123".to_string()))
        );
    }

    #[test]
    fn classify_unauth_health_status_matrix() {
        assert_eq!(
            classify_unauth_health_status(Some(200)),
            UnauthHealthClass::Open
        );
        assert_eq!(
            classify_unauth_health_status(Some(204)),
            UnauthHealthClass::Open
        );
        for code in [401_u16, 403, 404, 500] {
            assert_eq!(
                classify_unauth_health_status(Some(code)),
                UnauthHealthClass::Closed
            );
        }
        assert_eq!(
            classify_unauth_health_status(None),
            UnauthHealthClass::Inconclusive
        );

        let loopback_open = serve_auth_policy(UnauthHealthClass::Open, false);
        assert!(!loopback_open.advertise);
        assert!(loopback_open.keep_bind);

        let lan_open = serve_auth_policy(UnauthHealthClass::Open, true);
        assert!(!lan_open.advertise);
        assert!(!lan_open.keep_bind);

        for class in [UnauthHealthClass::Closed, UnauthHealthClass::Inconclusive] {
            for non_loopback in [false, true] {
                let policy = serve_auth_policy(class, non_loopback);
                assert!(policy.advertise);
                assert!(policy.keep_bind);
            }
        }
    }

    #[test]
    fn unauth_health_url_rewrites_unspecified_and_keeps_loopback() {
        assert_eq!(
            unauth_health_url("127.0.0.1:2419"),
            "http://127.0.0.1:2419/health"
        );
        assert_eq!(
            unauth_health_url("0.0.0.0:2419"),
            "http://127.0.0.1:2419/health"
        );
        let v6 = unauth_health_url("[::1]:2419");
        assert!(v6.contains("/health"));
        assert!(v6.contains("[::1]"));
        assert!(!unauth_health_url("0.0.0.0:2419").contains('?'));
        assert!(!unauth_health_url("[::]:2419").contains('?'));
        assert!(!unauth_health_url(":::2419").contains('?'));
    }

    #[test]
    fn unauth_health_probe_open_refuses_advertise() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let bind = format!("127.0.0.1:{}", addr.port());
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = std::io::Read::read(&mut stream, &mut buf);
                let _ = std::io::Write::write_all(
                    &mut stream,
                    b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n",
                );
            }
        });
        assert_eq!(probe_unauth_health(&bind), UnauthHealthClass::Open);
        assert!(!serve_auth_policy(UnauthHealthClass::Open, false).advertise);
    }

    #[test]
    fn unauth_health_probe_closed_allows_advertise() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let bind = format!("127.0.0.1:{}", addr.port());
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = std::io::Read::read(&mut stream, &mut buf);
                let _ = std::io::Write::write_all(
                    &mut stream,
                    b"HTTP/1.1 401 Unauthorized\r\nConnection: close\r\n\r\n",
                );
            }
        });
        assert_eq!(probe_unauth_health(&bind), UnauthHealthClass::Closed);
        let policy = serve_auth_policy(UnauthHealthClass::Closed, true);
        assert!(policy.advertise);
        assert!(policy.keep_bind);
    }

    #[test]
    fn unauth_health_probe_sends_no_secret() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let bind = format!("127.0.0.1:{}", addr.port());
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 8192];
                let n = std::io::Read::read(&mut stream, &mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let _ = std::io::Write::write_all(
                    &mut stream,
                    b"HTTP/1.1 401 Unauthorized\r\nConnection: close\r\n\r\n",
                );
                let _ = tx.send(req);
            }
        });
        let _ = probe_unauth_health(&bind);
        let req = rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("probe request");
        assert!(req.contains("GET /health"));
        assert!(!req.contains("server-key"));
        assert!(!req.contains("Authorization"));
        assert!(!req.contains("Cookie"));
        assert!(!req.contains("GROK_AGENT_SECRET"));
        assert!(!req.contains("GROK_SERVE_SECRET"));
        assert!(!req.contains("--secret"));
    }
}
