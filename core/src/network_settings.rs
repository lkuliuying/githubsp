use crate::error::{DownloadError, ErrorKind, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProxyMode {
    #[default]
    Direct,
    System,
    Custom,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NetworkSettings {
    pub proxy_mode: ProxyMode,
    pub proxy_url: String,
    /// 用户添加的镜像前缀，如 `https://example.com/`；下载时加在 GitHub 地址前。
    pub mirrors: Vec<String>,
}

/// 最多可添加的镜像数量。
pub const MAX_MIRRORS: usize = 8;

fn mirror_url(value: &str) -> Option<url::Url> {
    let url = url::Url::parse(value.trim()).ok()?;
    (url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url.path().ends_with('/'))
    .then_some(url)
}

impl NetworkSettings {
    /// 镜像线路：(线路标识, 主机名, 前缀)。标识按地址生成，删除其他镜像不会改变它。
    pub fn mirror_routes(&self) -> Vec<(String, String, String)> {
        self.mirrors
            .iter()
            .filter_map(|value| mirror_url(value))
            .map(|url| {
                let host = url.host_str().unwrap_or_default().to_owned();
                let path = url.path().trim_matches('/');
                let id = if path.is_empty() {
                    format!("mirror:{host}")
                } else {
                    format!("mirror:{host}/{path}")
                };
                (id, host, url.to_string())
            })
            .collect()
    }

    pub fn validate(&self) -> Result<()> {
        if self.mirrors.len() > MAX_MIRRORS {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                format!("最多添加 {MAX_MIRRORS} 个镜像地址"),
            ));
        }
        for value in &self.mirrors {
            if mirror_url(value).is_none() {
                return Err(DownloadError::new(
                    ErrorKind::InvalidInput,
                    format!("镜像地址「{value}」无效：须以 https:// 开头、以 / 结尾，且不含账号密码或查询参数"),
                ));
            }
        }
        let ids: std::collections::HashSet<_> = self
            .mirror_routes()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        if ids.len() != self.mirrors.len() {
            return Err(DownloadError::new(
                ErrorKind::InvalidInput,
                "镜像地址有重复",
            ));
        }
        self.validate_proxy()
    }

    fn validate_proxy(&self) -> Result<()> {
        if self.proxy_mode != ProxyMode::Custom && self.proxy_url.is_empty() {
            return Ok(());
        }
        let invalid = || {
            DownloadError::new(
                ErrorKind::InvalidInput,
                "代理地址须为不含账号、密码、路径或查询参数的 HTTP/HTTPS 地址",
            )
        };
        let url = url::Url::parse(&self.proxy_url).map_err(|_| invalid())?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || !matches!(url.path(), "" | "/")
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(invalid());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_proxy_modes_and_rejects_embedded_secrets_and_unsupported_schemes() {
        assert!(NetworkSettings::default().validate().is_ok());
        assert!(NetworkSettings {
            proxy_mode: ProxyMode::System,
            ..Default::default()
        }
        .validate()
        .is_ok());
        for url in ["http://127.0.0.1:7890", "https://[::1]:7890/"] {
            assert!(NetworkSettings {
                proxy_mode: ProxyMode::Custom,
                proxy_url: url.into(),
                ..Default::default()
            }
            .validate()
            .is_ok());
        }
        for url in [
            "",
            "socks5://localhost:7890",
            "http://user:password@localhost",
            "http://localhost/path",
            "http://localhost/?token=x",
        ] {
            assert!(NetworkSettings {
                proxy_mode: ProxyMode::Custom,
                proxy_url: url.into(),
                ..Default::default()
            }
            .validate()
            .is_err());
        }
    }

    #[test]
    fn mirrors_require_https_prefixes_and_get_stable_route_ids() {
        let settings = NetworkSettings {
            mirrors: vec!["https://a.example/".into(), "https://b.example/gh/".into()],
            ..Default::default()
        };
        assert!(settings.validate().is_ok());
        let routes = settings.mirror_routes();
        assert_eq!(routes[0].0, "mirror:a.example");
        assert_eq!(
            routes[1],
            (
                "mirror:b.example/gh".into(),
                "b.example".into(),
                "https://b.example/gh/".into()
            )
        );
        let network = crate::network::Network::production_with(&settings).unwrap();
        assert_eq!(network.routes.len(), 5);
        assert_eq!(
            network.routes[4].prefix.as_deref(),
            Some("https://b.example/gh/")
        );
        for bad in [
            "http://a.example/",
            "https://a.example/gh",
            "https://user:pw@a.example/",
            "https://a.example/?x=1",
            "not a url",
        ] {
            assert!(
                NetworkSettings {
                    mirrors: vec![bad.into()],
                    ..Default::default()
                }
                .validate()
                .is_err(),
                "{bad}"
            );
        }
        assert!(NetworkSettings {
            mirrors: vec!["https://a.example/".into(), "https://a.example/".into()],
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(NetworkSettings {
            mirrors: vec!["https://a.example/".into(); 9],
            ..Default::default()
        }
        .validate()
        .is_err());
    }

    #[tokio::test]
    async fn configured_proxy_handles_metadata_and_asset_hosts_without_direct_fallback() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let settings = NetworkSettings {
            proxy_mode: ProxyMode::Custom,
            proxy_url: format!("http://{}", listener.local_addr().unwrap()),
            ..Default::default()
        };
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for _ in 0..2 {
                let (socket, _) = listener.accept().await.unwrap();
                let mut reader = BufReader::new(socket);
                let mut first = String::new();
                reader.read_line(&mut first).await.unwrap();
                requests.push(first);
                reader.into_inner().write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
            }
            requests
        });
        let client = crate::network::Network::production_with(&settings)
            .unwrap()
            .client;
        for url in [
            "https://api.github.com/repos/o/r/releases",
            "https://github.com/o/r/releases/download/v1/tool.zip",
        ] {
            assert!(client
                .get(url)
                .timeout(std::time::Duration::from_secs(1))
                .send()
                .await
                .is_err());
        }
        let requests = tokio::time::timeout(std::time::Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
        assert!(requests[0].starts_with("CONNECT api.github.com:443 "));
        assert!(requests[1].starts_with("CONNECT github.com:443 "));
    }
}
