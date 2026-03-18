use std::borrow::Cow;
use testcontainers::Image;
use testcontainers::core::{ContainerPort, Mount, WaitFor};

#[derive(Debug, Clone)]
struct LiteServer {
    mounts: Vec<Mount>,
}

impl LiteServer {
    pub fn new() -> Self {
        Self {
            mounts: vec![Mount::volume_mount(
                "mylocalton-shared-volume",
                "/usr/share/data",
            )],
        }
    }
}

impl Image for LiteServer {
    fn name(&self) -> &str {
        "ghcr.io/neodix42/mylocalton-docker-lite-server"
    }

    fn tag(&self) -> &str {
        "v4.2.0"
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![]
    }

    fn env_vars(
        &self,
    ) -> impl IntoIterator<Item = (impl Into<Cow<'_, str>>, impl Into<Cow<'_, str>>)> {
        [
            ("LITE_SERVER_PORT", "30004"),
            ("CONSOLE_PORT", "30002"),
            ("PUBLIC_PORT", "30001"),
        ]
    }

    fn expose_ports(&self) -> &[ContainerPort] {
        &[ContainerPort::Tcp(30004)]
    }

    fn mounts(&self) -> impl IntoIterator<Item = &Mount> {
        self.mounts.iter()
    }
}

#[cfg(test)]
mod test {
    use crate::genesis::Genesis;
    use crate::liteserver::LiteServer;
    use testcontainers::core::{CmdWaitFor, ExecCommand};
    use testcontainers::runners::AsyncRunner;

    #[tokio::test]
    pub async fn test_liteserver_run() {
        let genesis = Genesis::new().start().await.unwrap();
        let container = LiteServer::new().start().await.unwrap();

        let result = container
            .exec(
                ExecCommand::new([
                    "/usr/local/bin/lite-client",
                    "-a",
                    "127.0.0.1:30004",
                    "-p",
                    "/var/ton-work/db/liteserver.pub",
                    "-t",
                    "3",
                    "-c",
                    "last",
                ])
                .with_cmd_ready_condition(CmdWaitFor::Exit { code: None }),
            )
            .await
            .unwrap();

        let stdout = container.stdout_to_vec().await.unwrap();
        let stdout = str::from_utf8(&stdout).unwrap();

        let stderr = container.stderr_to_vec().await.unwrap();
        let stderr = str::from_utf8(&stderr).unwrap();

        eprintln!("{}", stdout);
        eprintln!("{}", stderr);

        assert_eq!(result.exit_code().await.unwrap(), Some(0));
        assert!(false)
    }
}
