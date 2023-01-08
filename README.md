![Torb](./Torb.png =250x250)

Shoutout to my friend @SystemOverlord \[REAL NAME REDACTED\] for the anvil and sparks image for the org. This shoutout and the beer I have yet to get him satisfy our arrangement.

## What

Torb is a tool for quickly setting up best practice development infrastructure along with development stacks that have reasonably sane defaults. Instead of taking a couple hours to get a project started and then a week to get your infrastructure correct, do all of that in a couple minutes. 

## Mission

Make it simple and easy for software engineers to create and deploy infrastructure with best practices in place. The ideal would be 10 minutes or less to do so and the point where I'd consider Torb a failure is if it takes more than an hour to have dev, staging and prod with best practices in place.

In addition to the above Torb needs to provide an easy way of adding additional infrastructure and requirements as a project scales. On day one you probably have logs and something like Sentry or Rollbar but you might not have great CI/CD or more complex distributed tracing or bill of materials or an artifact repository or whatever. It should be stupid simple to add these to an existing system. Infrastructure needs change as a project changes and we should be flexible enough to accomodate that.

## Getting Started

First download the appropriate binary for your OS.

After that's downloaded you can either run Torb from your preferred directory or copy it somewhere on your PATH.

1.  Run `torb`. You'll see that the CLI is broken into nouns such as "repo" and "stack". Under each noun are the verbs that act upon it and let you do useful things.
2. Now run `torb init`. This will create a .torb folder located in your user's home directory. Inside of this we download a version of Terraform and pull our artifacts repo which contains community contributed Stacks, [Services](Torb#services) and [Projects](Torb#Projects). Finally this creates a `config.yaml` file which is where all of the CLI configuration is kept.
3.  Now you're ready to begin setting up a project using Torb.

## Configuring Torb

Earlier we mentioned a `config.yaml` file located in `~/.torb`, currently this file is pretty simple. It has two keys:

- githubToken - a PAT with access to read, write and admin.
- githubUser - The username of the user we are acting on behalf of.

## Repos

### Creating

Torb can create both a local repo and a repo on a service such as GitHub automatically. This:

- Creates the local folder
- Initializes Git, 
- Creates a blank README
- Creates the remote repository on GitHub. 
- Links local and remote
- Pushes first commit
 
Currently this doesn't happen under an organization, but that is on the list of things to tackle as config. It may be sufficient to provid and Organization token in the above config, but as of now it has not been tested.

**Note: Providing the full path for the local repo instead of just a name is currently required.**

	torb repo create ~/example/path/to/new_repo

This will create a local repo `new_repo` at the path provided and handle everything listed above.

## Stacks

### Checking-out and Initializing

#### Checking-out

First change directory into the repo where you'd like the stack to live.

Next list the available stacks with:

	torb stack list

This will output something like:

```
Torb Stacks:

- Flask App w/ React Frontend
- Rook Ceph Cluster
```

For this example we're going to choose `Flask App w/ React Frontend`

Run:

	torb stack checkout 'Flask App w/ React Frontend'

**Note: Depending on your shell you may need different quotes for names with spaces.**

This will produce a new file `stack.yaml` in your current directory, if you cat the file you can see the structure of a stack:

```
➜  test_repo git:(main) ✗ ls
stack.yaml

➜  test_repo git:(main) ✗ cat stack.yaml
version: v1.0.0
kind: stack
name: "Flask App w/ React Frontend"
description: "Production ready flask web app."
services:
  postgres_1:
    service: postgresql
    values: {}
    inputs:
      port: "5432"
      user: postgres
      password: postgres
      database: postgres
      num_replicas: "1"
  nginx_ingress_controller_1:
    service: nginx-ingress-controller
    values:
      controller:
        admissionWebhooks:
          enabled: false
    inputs: {}
projects:
  flaskapp_1:
    project: flaskapp
    inputs:
      name: flaskapp
      db_host: self.service.postgres_1.output.host
      db_port: self.service.postgres_1.output.port
      db_user: self.service.postgres_1.output.user
      db_pass: self.service.postgres_1.output.password
      db_database: self.service.postgres_1.output.database
    values: {}
    build:
      tag: latest
      registry: ""
    deps:
      services:
        - postgres_1
  createreactapp_1:
    project: createreactapp
    inputs:
      name: createreactapp
      ingress: "true"
    values:
      ingress:
        hosts:
          - name: localhost
            path: /
            servicePort: "8000"
      extraEnvVars:
        - name: API_HOST
          value: self.project.flaskapp_1.output.host
        - name: API_PORT
          value: "5000"
    build:
      tag: latest
      registry: ""
    deps:
      projects:
        - flaskapp_1
      services:
        - nginx_ingress_controller_1
```

A stack is comprised of `services` and `projects` as the basic units. We won't go into their structure deeply here, but approximately a service is something a user would just configure and deploy and a project is anything where a user is modifying source and building.

Each stack is a DAG and dependencies can either be explicitly listed as they are above or Torb can figure them out implicitly based on references in the inputs sections and the values overrides in any of the units. Each unit in a stack is referenced internally by it's fully qualified name comprised of <stack_name>.<unit_type>.<unit_name>

When a stack is initialized, built or deployed the dependency chain is walked to the end and executed, this is then unwound all the way to the initial starting unit(s).

#### Initializing

After you've checked out a stack you need to initialize it before you can proceed to build and deploy the stack. Each unit can in it's definition include an initialization step to help set it up in your project. Most of the time for `projects` this means creating the folder, running a generator of somekind to create default code and copying over any config or build files it will need. If you need to examine a particular unit to see what it does you can check it out in [Torb Artifacts](https://github.com/TorbFoundry/torb-artifacts)

To initialize your stack run:

	torb stack init stack.yaml

With the stack that we're using your repo will look something like this:

```
➜  test_repo git:(main) ✗ ls
createreactapp flaskapp       flaskapp_venv  stack.yaml
```

Each folder is the name of the unit in [Torb Artifacts](https://github.com/TorbFoundry/torb-artifacts), eventually you'll be able to name these whatever you want but for now they have to match the unit name. 

Depending on the unit, such as this react app, you'll need to build the artifact like `npm build` before you are able to deploy. Go ahead and change directory into `createreactapp` and run `npm run build`. Torb will not install programming languages, libraries or anything else for working with projects so make sure you have these things installed.

If you need to install npm and node for this tutorial, you can follow their [guide](https://docs.npmjs.com/downloading-and-installing-node-js-and-npm)

Next we'll look at building and deploying.

### Building and Deploying

#### Building

Currently Torb supports building an image with Docker, or a build script, but not both. The resulting artifact must be a docker image regardless. Using a build script will let you do additional steps such as have torb automatically run the `npm run build` step from above in addition to building the image.

Building your stack will recurse through the graph and run whatever is configured in the `build` section for the individual unit defined in the `stack.yaml` 

```
flaskapp_1:
    project: flaskapp
    inputs:
      name: flaskapp
      db_host: self.service.postgres_1.output.host
      db_port: self.service.postgres_1.output.port
      db_user: self.service.postgres_1.output.user
      db_pass: self.service.postgres_1.output.password
      db_database: self.service.postgres_1.output.database
    values: {}
    build:
      tag: latest
      registry: ""
    deps:
      services:
        - postgres_1
```

**Note: To use a script instead, set script_path instead of tag and registry.**

You can see in the above unit that build is configured to tag the docker image with `latest` and since the registry is empty it will push the image to the default docker hub repository you are currently signed in to.

If you just want to have the image locally and skip pushing to a registry you can change registry to `local`. This is useful is you're running a kubernetes cluster that can read your local docker images like the cluster that can be enabled with Docker Destkop on mac and wsl. 

If you're running a kubernetes cluster on a remote server you will need to make sure the appropriate registry is configured here and that you are authenticated to it as this will also be used to pull the image on your cluster later on.

**Note: If you're using Minikube you will either need to use remote registry or load the local image with `minikube image load <IMAGE_NAME>`**

To build your stack run

	torb stack build stack.yaml

Expect the first build to take some time as this will be building the docker images from scratch.

If all goes well you should see output for the main IAC (Terraform) file torb generates for it's internal build state.

**Note: All build state is kept in a hidden folder .torb_buildstate in your repo. Currently this isn't intended to be exposed to users, but that may change in the future. We want to add eject functionality if people choose to opt out of using Torb and at that time this will be more up front.***


#### Deploying

##### Foreword

Torb currently deploys only to Kubernetes, we use Terraform to handle the deploy and bundle a version of terraform in the .torb folder in your home directory. This is so we can control what version Torb is using and keep it separate from any other version of Terraform you might already be using on your system.

Torb respects the `KUBECONFIG` env var and if that is not set we default to `~/.kube/config` whatever your active context is will be used so make sure you're set to the right cluster. This also makes us fairly cluster agnostic as we rely on you to have access and connectivity to the cluster you want to deploy to.

Deploys respect the dependency ordering set in the `stack.yaml` we use the same method for detecing implicit and explicit depencies in Torb.

There are some tricky aspects of the deploy, we rely on the `helm provider` in Terraform and Helm in general to deploy to Kube. Helm itself is a good tool and handles a lot of complexities of putting together a set of artifacts in a convenient bundle, but is fairly limited and opaque when it comes to handling errors, timeouts, dealing with data persistance etc during a deploy. In that case it really is only a vehicle for applying a chart. This means we are limited by Helm AND by the respective chart maintainers in our artifacts.

As an example, if the chart being applied isn't useing StatefulSets and includes PersistentVolumeClaims your PVC will be deleted when the chart is cleaned up. In a lot of ways it may be better to create a separate PVC under a StatefulSet in addition to the existing Deployment based chart and see if the chart supports passing a reference to that claim, versus relying on them to do the correct thing for your usecase. 

Torb does not at this moment have a way to enforce these practices but as we grow can put requirements in place for our artifacts that will help here. Hopefully this isn't too often exposed to you as end users, but is a concern for anyone who is creating stacks and units under [Torb Artifacts](https://github.com/TorbFoundry/torb-artifacts)

Longer term we may work on something to replace using Helm while trying to support the chart format itself but for now it's the best we have. As an example we've looked into Kustomize and handling releases ourselves but need to further evaluate how much we will lose out on from the Helm ecosystem.

##### Deploy

To deploy with Torb run

	torb stack deploy stack.yaml

You should see Terraform initialize a workspace and begin to apply a plan.

At this point you can wait until things finish or use Kubectl to check the status of the deployment. The namespace being deployed to can be configured at the stack level and a per unit level in the `stack.yaml`.

Currently we are using a local backend for Terraform but do plan to support popular cloud providers, and our own cloud solution.

If all is good you will eventually see a success message from Terraform with a list of new infrastructure created, changed or removed.

In the event of an issue the default timeout is 5 minutes and you can safely clean up releases in Helm without impacting Torb.



