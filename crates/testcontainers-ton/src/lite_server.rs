use std::borrow::Cow;
use std::collections::HashMap;
use testcontainers::core::{ContainerPort, WaitFor};
use testcontainers::{CopyDataSource, CopyToContainer, Image};

#[derive(Debug, Clone)]
pub struct LiteServer {
    copy_to_container: Vec<CopyToContainer>,
}

impl LiteServer {
    pub fn new(config: Vec<u8>) -> Self {
        Self {
            copy_to_container: vec![CopyToContainer::new(
                CopyDataSource::Data(config),
                "/usr/share/data/global.config.json",
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

    // TODO[akostylev0]: add HEALTHCHECK in Dockerfile
    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![
            WaitFor::message_on_either_std("finished downloading state"),
            WaitFor::message_on_either_std(
                "Broadcast_benchmark deserialize_block_broadcast block_id=(-1,8000000000000000,9)",
            ),
        ]
    }

    fn env_vars(
        &self,
    ) -> impl IntoIterator<Item = (impl Into<Cow<'_, str>>, impl Into<Cow<'_, str>>)> {
        let mut envs = HashMap::new();
        envs.insert("LITE_SERVER_PORT".to_owned(), "30004".to_owned());
        envs.insert("CONSOLE_PORT".to_owned(), "30002".to_owned());
        envs.insert("PUBLIC_PORT".to_owned(), "30001".to_owned());
        envs.insert("VERBOSITY".to_owned(), "3".to_owned());

        envs
    }

    fn copy_to_sources(&self) -> impl IntoIterator<Item = &CopyToContainer> {
        self.copy_to_container.iter()
    }

    fn expose_ports(&self) -> &[ContainerPort] {
        &[ContainerPort::Tcp(30004), ContainerPort::Tcp(40004)]
    }
}

#[cfg(test)]
mod integration {
    use crate::genesis::Genesis;
    use crate::lite_server::LiteServer;
    use testcontainers::core::{CmdWaitFor, ExecCommand};
    use testcontainers::runners::AsyncRunner;

    #[tokio::test]
    pub async fn test_liteserver_run() {
        let genesis = Genesis::default().start().await.unwrap();
        let mut config = vec![];
        genesis
            .copy_file_from("/usr/share/data/global.config.json", &mut config)
            .await
            .unwrap();
        let container = LiteServer::new(config).start().await.unwrap();

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

        assert_eq!(result.exit_code().await.unwrap(), Some(0));
    }
}
