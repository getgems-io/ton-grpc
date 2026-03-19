use std::borrow::Cow;
use std::collections::HashMap;
use testcontainers::Image;
use testcontainers::core::{ContainerPort, WaitFor};

#[derive(Debug, Clone, Default)]
pub struct Genesis {}

impl Image for Genesis {
    fn name(&self) -> &str {
        "ghcr.io/neodix42/mylocalton-docker"
    }

    fn tag(&self) -> &str {
        "v4.2.0"
    }

    // TODO[akostykev0]: add HEALTHCHECK in Dockerfile
    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_either_std(
            "Done importing neighbor msg queues for shard",
        )]
    }

    fn env_vars(
        &self,
    ) -> impl IntoIterator<Item = (impl Into<Cow<'_, str>>, impl Into<Cow<'_, str>>)> {
        let mut envs = HashMap::new();
        envs.insert("GENESIS", "true");
        envs.insert("NAME", "genesis");
        envs.insert("VERBOSITY", "3");

        envs
    }

    fn expose_ports(&self) -> &[ContainerPort] {
        &[
            ContainerPort::Tcp(40004),
            ContainerPort::Tcp(40002),
            ContainerPort::Udp(40003),
            ContainerPort::Udp(40001),
            ContainerPort::Tcp(8888),
        ]
    }
}

#[cfg(test)]
mod test {
    use crate::genesis::Genesis;
    use testcontainers::core::{CmdWaitFor, ExecCommand};
    use testcontainers::runners::AsyncRunner;

    #[tokio::test]
    #[ignore = "requires docker"]
    pub async fn test_genesis_run() {
        let genesis = Genesis::default().start().await.unwrap();

        let result = genesis
            .exec(
                ExecCommand::new(vec![
                    "/usr/local/bin/lite-client",
                    "-a",
                    "127.0.0.1:40004",
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
