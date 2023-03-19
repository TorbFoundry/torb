// Business Source License 1.1
// Licensor:  Torb Foundry
// Licensed Work:  Torb v0.3.5-03.13
// The Licensed Work is © 2023-Present Torb Foundry
//
// Change License: GNU Affero General Public License Version 3
// Additional Use Grant: None
// Change Date: Feb 22, 2023
//
// See LICENSE file at https://github.com/TorbFoundry/torb/blob/main/LICENSE for details.

use crate::artifacts::{write_build_file, ArtifactRepr};
use crate::builder::StackBuilder;
// use crate::deployer::StackDeployer;
use crate::composer::Composer;
use crate::deployer::StackDeployer;
use crate::utils::buildstate_path_or_create;
use crate::utils::{
    get_resource_kind, CommandConfig, CommandPipeline, PrettyContext, PrettyExit, ResourceKind,
};

use std::sync::{Arc, Mutex, MutexGuard};
use std::{sync::PoisonError, time::Duration};
use tokio::{
    runtime::Runtime,
    sync::mpsc::{channel, Receiver},
    time,
};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WatcherConfig {
    paths: Vec<String>,
    interval: u64,
    patch: bool,
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
    pub build_hash: String,
    pub build_filename: String,
    internal: Arc<WatcherInternal>,
}

struct WatcherInternal {
    pub queue: Mutex<Vec<Event>>,
    pub separate_local_registry: bool,
}

impl WatcherInternal {
    fn new(separate_local_registry: bool) -> Self {
        WatcherInternal {
            queue: Mutex::new(Vec::<Event>::new()),
            separate_local_registry,
        }
    }
    fn redeploy(
        &self,
        artifact: Arc<ArtifactRepr>,
    ) -> Result<(), PoisonError<MutexGuard<Vec<Event>>>> {
        self.queue.lock().map(|mut queue| {
            if !queue.is_empty() {
                println!("Changes found during watcher interval, redeploying!");

                queue.clear();
                queue.shrink_to(10);

                let build_platforms = "".to_string();

                let mut builder = StackBuilder::new(&artifact, build_platforms, false, self.separate_local_registry.clone());

                builder.build().use_or_pretty_error(
                    false,
                    PrettyContext::default()
                    .success("Success! Watcher rebuilt stack.")
                    .error("Oh no! The Watcher failed to rebuild the stack. Continuing to watch, please fix your errors.")
                    .pretty()
                );

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

        let location = std::path::Path::new("/tmp").to_path_buf();

        let (build_hash, build_filename, artifact) = write_build_file(contents, Some(&location));
        let watcher = artifact.watcher.clone();

        Watcher::new(
            watcher.paths,
            artifact,
            Some(watcher.interval),
            Some(watcher.patch),
            local_registry,
            build_hash,
            build_filename,
        )
    }

    fn new(
        paths: Vec<String>,
        artifact: ArtifactRepr,
        interval: Option<u64>,
        patch: Option<bool>,
        local_registry: bool,
        build_hash: String,
        build_filename: String,
    ) -> Self {
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
            build_hash,
            build_filename,
            internal,
        }
    }

    fn setup_stack(&mut self) {
        let build_platforms = "".to_string();

        let mut builder = StackBuilder::new(
            &self.artifact,
            build_platforms,
            false,
            self.internal.separate_local_registry.clone(),
        );

        builder.build().use_or_pretty_exit(
            PrettyContext::default()
            .error("Oh no, we were unable to build the stack when starting the watcher!")
            .success("Success! Stack has been built!")
            .context("Errors here are typically because of a failed docker build, syntax issue in the dockerfile or a connectivity issue with the docker registry.")
            .suggestions(vec![
                "Check that your dockerfile has no syntax errors and is otherwise correct.",
                "If you're building with an image registry that is hosted on the same machine, but as a separate service and not the default docker registry, try passing --local-hosted-registry as a flag."
            ])
            .pretty()
        );

        let mut composer =
            Composer::new(self.build_hash.clone(), &self.artifact, self.patch.clone());
        composer.compose().unwrap();

        let mut deployer = StackDeployer::new(self.patch.clone());

        deployer
            .deploy(&self.artifact, false)
            .use_or_pretty_exit(
                PrettyContext::default()
                .error("Oh no, we were unable to deploy the stack when starting the watcher!")
                .success("Success! Stack has been deployed!")
                .context("Errors here are typically because of failed Terraform deployments or Helm failures.")
                .suggestions(vec![
                    "Check that your Terraform IaC environment was generated correctly. \nThis can be found in your project folder at, .torb_buildstate/iac_environment, or .torb_buildstate/watcher_iac_environment if you're using the watcher.",
                    "To see if your Helm deployment failed you can do `helm ls --namespace <namespace>` where the namespace is the one you're deploying to.",
                    "After seeing if the deployment has failed in Helm, you can use kubectl to debug further. Take a look at https://kubernetes.io/docs/reference/kubectl/cheatsheet/ if you're less familiar with kubectl."
                ])
                .pretty()
            );

        let buildstate_path = buildstate_path_or_create();
        let non_watcher_iac = buildstate_path.join("iac_environment");
        let watcher_iac = buildstate_path.join("watcher_iac_environment");
        let tf_state_path = watcher_iac.join("terraform.tfstate");

        if tf_state_path.exists() {
            let new_path = non_watcher_iac.join("terraform.tfstate");
            std::fs::copy(tf_state_path, new_path).expect("Failed to copy supporting build file.");
        };
    }

    pub fn start(mut self) {
        self.setup_stack();

        let rt = Runtime::new().unwrap();
        let interval = self.interval.clone();

        let internal_ref = self.internal.clone();
        let artifact_ref = self.artifact.clone();
        rt.spawn(async move {
            let mut interval = time::interval(Duration::from_millis(interval.to_owned()));
            loop {
                interval.tick().await;
                internal_ref
                    .redeploy(artifact_ref.clone())
                    .expect("Unable to complete redeploy!");
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

    fn async_watcher(
        &self,
    ) -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
        let (tx, rx) = channel(1);

        let watcher = RecommendedWatcher::new(
            move |res| {
                let rt = Runtime::new().unwrap();

                rt.block_on(async {
                    tx.send(res).await.unwrap();
                })
            },
            Config::default(),
        )?;

        Ok((watcher, rx))
    }
}
