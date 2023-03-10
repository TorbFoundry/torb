// Business Source License 1.1
// Licensor:  Torb Foundry
// Licensed Work:  Torb v0.3.0-02.22
// The Licensed Work is © 2023-Present Torb Foundry
//
// Change License: GNU Affero General Public License Version 3
// Additional Use Grant: None
// Change Date: Feb 22, 2023
//
// See LICENSE file at https://github.com/TorbFoundry/torb/blob/main/LICENSE for details.

use crate::artifacts::{deserialize_stack_yaml_into_artifact, ArtifactRepr};
use crate::builder::StackBuilder;
// use crate::deployer::StackDeployer;
use crate::utils::{CommandConfig, get_resource_kind, ResourceKind, CommandPipeline};

use tokio::{
    sync::mpsc::{channel, Receiver},
    runtime::Runtime,
    time
};
use std::{time::{Duration}, sync::PoisonError};
use std::sync::{Arc, Mutex, MutexGuard};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher, Config};
use std::path::{PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WatcherConfig {
    paths: Vec<String>,
    interval: u64,
    patch: bool
}

impl Default for WatcherConfig {
    fn default() -> WatcherConfig {
        WatcherConfig {
            paths: vec!["./".to_string()],
            interval: 3000,
            patch: true,
        }
    }
}

pub struct Watcher {
    pub paths: Vec<PathBuf>,
    pub interval: u64,
    pub patch: bool,
    pub artifact: Arc<ArtifactRepr>,
    internal: Arc<WatcherInternal>
}

struct WatcherInternal {
    pub queue: Mutex<Vec<Event>>,
    pub separate_local_registry: bool
}

impl WatcherInternal {
    fn new(separate_local_registry: bool) -> Self {
        WatcherInternal { queue: Mutex::new(Vec::<Event>::new()), separate_local_registry }
    }
    fn redeploy(&self, artifact: Arc<ArtifactRepr>) -> Result<(), PoisonError<MutexGuard<Vec<Event>>>> {
        self.queue.lock().map(|mut queue| {
            if !queue.is_empty() {
                println!("Changes found during watcher interval, redeploying!");

                queue.clear();
                queue.shrink_to(10);

                let build_platforms = "linux/amd64,linux/arm64".to_string();

                let mut builder = StackBuilder::new(&artifact, build_platforms, false, self.separate_local_registry.clone());

                builder.build().expect("Failed to build stack during watcher redeploy.");

                for (_, node) in artifact.nodes.iter() {
                    let resource_name = format!("{}-{}", artifact.release(), node.display_name(Some(true)));

                    let namespace = artifact.namespace(node);
                    let kind_res = get_resource_kind(&resource_name, &namespace);

                    let kind = match kind_res {
                        Err(err) => {
                            panic!("{}", err)
                        }
                        Ok(_enum) => {
                            match _enum {
                                ResourceKind::DaemonSet => "daemonset",
                                ResourceKind::Deployment => "deployment",
                                ResourceKind::StatefulSet => "statefulset"
                            }
                        }
                    };

                    let cmd = CommandConfig::new("kubectl",
                    vec![
                            "rollout",
                            "restart",
                            kind,
                            resource_name.as_str(),
                            "--namespace",
                            &namespace
                        ],
                        None
                    );
                    let err_msg = format!("Unable to execute rollout redeploy for {} {}", kind, resource_name);
                    CommandPipeline::execute_single(cmd).expect(&err_msg);
                }

            }
        })
    }
}

impl Watcher {
    pub fn configure(file_path: String, local_registry: bool) -> Self {
        let contents = std::fs::read_to_string(file_path)
            .expect("Something went wrong reading the stack file.");

        let artifact = deserialize_stack_yaml_into_artifact(&contents)
            .expect("Unable to read stack file into internal representation.");

        let watcher = artifact.watcher.clone();


        Watcher::new(watcher.paths, artifact, Some(watcher.interval), Some(watcher.patch), local_registry)
    }

    fn new(paths: Vec<String>, artifact: ArtifactRepr, interval: Option<u64>, patch: Option<bool>, local_registry: bool) -> Self {
        let interval = interval.unwrap_or(3000);
        let patch = patch.unwrap_or(true);
        let mut bufs = Vec::new();

        for str in paths.iter() {
            let p = PathBuf::from(str);
            bufs.push(p);
        }

        let internal = Arc::new(WatcherInternal::new(local_registry));

        Watcher {
            paths: bufs,
            interval,
            patch,
            artifact: Arc::new(artifact),
            internal
        }
    }

    pub fn start(mut self) {
        let rt = Runtime::new().unwrap();
        let interval = self.interval.clone();

        let internal_ref = self.internal.clone();
        let artifact_ref = self.artifact.clone();
        rt.spawn(async move {
            let mut interval = time::interval(Duration::from_millis(interval.to_owned()));
            loop {
                interval.tick().await;
                internal_ref.redeploy(artifact_ref.clone()).expect("Unable to complete redeploy!");
            }
        });

        rt.block_on(async {
            if let Err(e) = self.watch().await {
                println!("error: {:?}", e)
            }
        });

        rt.shutdown_timeout(Duration::from_millis(2000))
    }

    async fn watch(&mut self) -> notify::Result<()> {
        let (mut watcher, mut rx) = self.async_watcher()?;

        for path in self.paths.iter() {
            println!("Watching: {}", path.to_str().unwrap());
            watcher.watch(&path, RecursiveMode::Recursive)?;
        }

        while let Some(res) = rx.recv().await {
            match res {
                Ok(event) => self.internal.queue.lock()?.push(event),
                Err(e) => panic!("{}", e),
            }
        }

        Ok(())
    }

    fn async_watcher(&self) -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
        let (tx, rx) = channel(1);

        let watcher = RecommendedWatcher::new(move |res| {

            let rt = Runtime::new().unwrap();

            rt.block_on(async {
                    tx.send(res).await.unwrap();
            })

        }, Config::default())?;

        Ok((watcher, rx))
    }
}