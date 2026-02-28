//! Docker sandbox provider — runs a local container as a sandbox.
//!
//! Uses [`bollard`] to manage Docker containers. No external API key required;
//! only a running Docker daemon is needed. Preview URLs are `http://localhost:{port}`
//! and are only accessible from the machine running ZeroBuild.

use super::{CommandOutput, SandboxClient};
use async_trait::async_trait;
use bollard::container::{
    Config as ContainerConfig, CreateContainerOptions, LogOutput, RemoveContainerOptions,
    StartContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::image::CreateImageOptions;
use bollard::Docker;
use futures_util::StreamExt;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Directories skipped when collecting a snapshot.
const SKIP_DIRS: &[&str] = &["node_modules", ".next", ".git", "dist", "build", ".cache"];

/// Docker-backed sandbox client.
///
/// Creates a container from the configured image with a mapped host port,
/// then executes commands via `docker exec`.
pub struct DockerSandboxClient {
    docker: Docker,
    image: String,
    container_id: Arc<Mutex<Option<String>>>,
    /// Actual host port mapped from container port 3000.
    host_port: Arc<Mutex<Option<u16>>>,
}

impl DockerSandboxClient {
    /// Create a new client using the default Docker socket path.
    pub fn new(image: &str) -> anyhow::Result<Self> {
        let docker = Docker::connect_with_defaults()
            .map_err(|e| anyhow::anyhow!("Failed to connect to Docker: {e}"))?;
        Ok(Self {
            docker,
            image: image.to_string(),
            container_id: Arc::new(Mutex::new(None)),
            host_port: Arc::new(Mutex::new(None)),
        })
    }
}

#[async_trait]
impl SandboxClient for DockerSandboxClient {
    async fn create_sandbox(
        &self,
        reset: bool,
        _template: &str,
        _timeout_ms: u64,
    ) -> anyhow::Result<String> {
        if !reset {
            if let Some(id) = self.container_id.lock().clone() {
                return Ok(id);
            }
        }

        // Kill existing container if present
        let old_id = self.container_id.lock().clone();
        if let Some(id) = old_id {
            let _ = self
                .docker
                .remove_container(
                    &id,
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                )
                .await;
        }
        *self.container_id.lock() = None;
        *self.host_port.lock() = None;

        // Pull image (drain stream, ignore "already present" errors)
        let mut pull_stream = self.docker.create_image(
            Some(CreateImageOptions {
                from_image: self.image.as_str(),
                ..Default::default()
            }),
            None,
            None,
        );
        while let Some(item) = pull_stream.next().await {
            // Log pull progress; ignore individual errors (layer already exists, etc.)
            if let Err(e) = item {
                tracing::debug!("Docker pull stream item: {e}");
            }
        }

        // Map container port 3000 to a random host port
        let port_bindings: HashMap<String, Option<Vec<bollard::models::PortBinding>>> = [(
            "3000/tcp".to_string(),
            Some(vec![bollard::models::PortBinding {
                host_ip: Some("127.0.0.1".to_string()),
                host_port: Some("0".to_string()),
            }]),
        )]
        .into_iter()
        .collect();

        let host_config = bollard::models::HostConfig {
            port_bindings: Some(port_bindings),
            ..Default::default()
        };

        let container = self
            .docker
            .create_container(
                None::<CreateContainerOptions<&str>>,
                ContainerConfig {
                    image: Some(self.image.as_str()),
                    cmd: Some(vec!["sleep", "infinity"]),
                    exposed_ports: Some({
                        let mut m = HashMap::new();
                        m.insert("3000/tcp", HashMap::new());
                        m
                    }),
                    host_config: Some(host_config),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create Docker container: {e}"))?;

        let container_id = container.id.clone();

        self.docker
            .start_container(&container_id, None::<StartContainerOptions<&str>>)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to start Docker container: {e}"))?;

        // Inspect to get the actual mapped host port
        let inspect = self
            .docker
            .inspect_container(&container_id, None)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to inspect container: {e}"))?;

        let host_port = inspect
            .network_settings
            .as_ref()
            .and_then(|ns| ns.ports.as_ref())
            .and_then(|ports| ports.get("3000/tcp"))
            .and_then(|bindings| bindings.as_ref())
            .and_then(|bindings| bindings.first())
            .and_then(|b| b.host_port.as_ref())
            .and_then(|p| p.parse::<u16>().ok())
            .ok_or_else(|| anyhow::anyhow!("Could not determine mapped host port for container"))?;

        *self.container_id.lock() = Some(container_id.clone());
        *self.host_port.lock() = Some(host_port);

        Ok(container_id)
    }

    async fn kill_sandbox(&self) -> anyhow::Result<String> {
        let container_id = match self.container_id.lock().clone() {
            Some(id) => id,
            None => return Ok("No active container to kill.".to_string()),
        };

        self.docker
            .remove_container(
                &container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to remove Docker container: {e}"))?;

        *self.container_id.lock() = None;
        *self.host_port.lock() = None;

        Ok(format!("Container {container_id} removed."))
    }

    async fn run_command(
        &self,
        command: &str,
        workdir: &str,
        timeout_ms: u64,
    ) -> anyhow::Result<CommandOutput> {
        let container_id = self
            .container_id
            .lock()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active container"))?;

        let exec_id = self
            .docker
            .create_exec(
                &container_id,
                CreateExecOptions {
                    cmd: Some(vec!["sh", "-c", command]),
                    working_dir: Some(workdir),
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create exec: {e}"))?
            .id;

        let mut stdout_buf = String::new();
        let mut stderr_buf = String::new();

        let start_result = tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            async {
                match self
                    .docker
                    .start_exec(&exec_id, None)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to start exec: {e}"))?
                {
                    StartExecResults::Attached { mut output, .. } => {
                        while let Some(chunk) = output.next().await {
                            match chunk
                                .map_err(|e| anyhow::anyhow!("Exec output error: {e}"))?
                            {
                                LogOutput::StdOut { message } => {
                                    stdout_buf.push_str(
                                        &String::from_utf8_lossy(&message),
                                    );
                                }
                                LogOutput::StdErr { message } => {
                                    stderr_buf.push_str(
                                        &String::from_utf8_lossy(&message),
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                    StartExecResults::Detached => {}
                }
                Ok::<(), anyhow::Error>(())
            },
        )
        .await;

        match start_result {
            Err(_elapsed) => {
                return Err(anyhow::anyhow!(
                    "Command timed out after {timeout_ms}ms"
                ))
            }
            Ok(Err(e)) => return Err(e),
            Ok(Ok(())) => {}
        }

        let inspect = self
            .docker
            .inspect_exec(&exec_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to inspect exec: {e}"))?;

        let exit_code = inspect.exit_code.unwrap_or(0);

        Ok(CommandOutput {
            stdout: stdout_buf,
            stderr: stderr_buf,
            exit_code,
        })
    }

    async fn write_file(&self, path: &str, content: &str) -> anyhow::Result<()> {
        let dir = std::path::Path::new(path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("/");

        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, content);

        // For files up to ~500KB, write in one command.
        // printf is more portable than echo for binary/special-char content.
        let cmd = format!(
            "mkdir -p '{dir}' && printf '%s' '{b64}' | base64 -d > '{path}'"
        );

        let out = self.run_command(&cmd, "/", 30_000).await?;

        if out.exit_code != 0 {
            anyhow::bail!(
                "write_file failed (exit {}): {}",
                out.exit_code,
                out.stderr
            );
        }

        Ok(())
    }

    async fn read_file(&self, path: &str) -> anyhow::Result<String> {
        let out = self
            .run_command(&format!("base64 -w0 '{path}'"), "/", 30_000)
            .await?;

        if out.exit_code != 0 {
            anyhow::bail!("File not found or unreadable: {path}");
        }

        let bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            out.stdout.trim(),
        )
        .map_err(|e| anyhow::anyhow!("base64 decode failed: {e}"))?;

        String::from_utf8(bytes).map_err(|e| anyhow::anyhow!("UTF-8 decode failed: {e}"))
    }

    async fn list_files(&self, path: &str) -> anyhow::Result<String> {
        let out = self
            .run_command(&format!("ls -la '{path}' 2>&1"), "/", 10_000)
            .await?;

        Ok(out.stdout)
    }

    async fn get_preview_url(&self, _port: u16) -> anyhow::Result<String> {
        let host_port = self
            .host_port
            .lock()
            .ok_or_else(|| anyhow::anyhow!("No active container"))?;

        Ok(format!("http://localhost:{host_port}"))
    }

    async fn collect_snapshot_files(
        &self,
        workdir: &str,
    ) -> anyhow::Result<HashMap<String, String>> {
        let skip_pattern = SKIP_DIRS
            .iter()
            .map(|d| format!("-not -path '*/{d}/*'"))
            .collect::<Vec<_>>()
            .join(" ");

        let find_cmd = format!("find '{workdir}' -type f {skip_pattern}");
        let out = self.run_command(&find_cmd, "/", 60_000).await?;

        let mut files = HashMap::new();
        for file_path in out.stdout.lines() {
            let file_path = file_path.trim();
            if file_path.is_empty() {
                continue;
            }
            match self.read_file(file_path).await {
                Ok(content) => {
                    files.insert(file_path.to_string(), content);
                }
                Err(e) => {
                    tracing::debug!("Skipping unreadable file {file_path}: {e}");
                }
            }
        }

        Ok(files)
    }

    fn current_id(&self) -> Option<String> {
        self.container_id.lock().clone()
    }

    fn set_id(&self, id: String) {
        *self.container_id.lock() = Some(id);
    }

    fn clear_id(&self) {
        *self.container_id.lock() = None;
        *self.host_port.lock() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_client_has_no_id() {
        // DockerSandboxClient::new() may fail if Docker is not available.
        // We only test construction here, not Docker connectivity.
        if let Ok(client) = DockerSandboxClient::new("node:20-alpine") {
            assert!(client.current_id().is_none());
        }
    }

    #[test]
    fn set_and_clear_id() {
        if let Ok(client) = DockerSandboxClient::new("node:20-alpine") {
            client.set_id("container-123".to_string());
            assert_eq!(client.current_id().as_deref(), Some("container-123"));
            client.clear_id();
            assert!(client.current_id().is_none());
        }
    }
}
