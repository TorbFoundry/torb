# Stacks

## Description

## Spec

### Version

- Type: String
- Value: `v1.0.0`
- Description: "Version of the stack specification, currently only 1 version exists, all new features are added to this specification."
- Required: True

### Kind

- Type: String
- Value: `stack | service | project`
- Description: "Specifies what type of configuration this is for Torb. For stacks this should be set to 'stack'"
- Required: True

### Name

- Type: String
- Value: `Defined by stack developer`
- Description: "The name of the stack as defined in the artifact repository template it was created from."
- Required: True

### Description

- Type: String
- Value `Defined by stack developer`
- Description: "A human readable description of the stack as defined by the stack developer."
- Required: True

### Release

- Type: String
- Value: `User Defined Value`
- Description: "Optional configuration to specifiy a helm release name, if not provided a random name will be generated for each change, this will result in tearing down of the environment and creating a new release. Specify this option to update an existing release without"
- Required: False

### Watcher

- Type: Mapping[String, Any]
- Description: ""
- Required: False

    #### Mapping
    
    ##### paths

    - Type: List[String]
    - Value: ""
    - Description: "A list of paths for the watcher to check for changes."
    - Required: True
    
    ##### patch

    - Type: Bool
    - Value: True
    - Description: "If true, patches helm charts to always pull their image, this is needed to redeploy detected changes."
    - Required: True

    ##### interval

    - Type: Int
    - Value: 3000
    - Description: "The interval in miliseconds the watcher will check for changes having occured."
    - Required: True

### Services

- Type: Mapping[String, Service]
- Value: `User Defined Value`
- Description: "The services section specifies all services Torb will configure, build and deploy to your Kubernetes cluster."
- Required: False

    ### Service

    - Type: Mapping[String, Any]
    - Value: `User Defined Value`
    - Description: "An individual service configuration. "
    - Required: True

        #### Mapping

        #### service

        - Type: String
        - Value: `Torb Service Name`
        - Description: "The name of the service defined in an artifact repository."
        - Required: True
        
        #### values

        - Type: Mapping[String, Any]
        - Value: Any value found in the associated helm chart for this service.
        - Description: "Optional mapping, this can be any value found in the associated helm chart for this service, it will be passed through directly."
        - Required: False
        
        #### inputs

        - Type: Mapping[String, String | Numeric | Array | Bool]
        - Value: `User Defined`
        - Description: "A mapping of input names to values as defined by the service or project. If values are not supplied Torb will use the defaults set in the service or project by the service or project template maintainer."
        - Required: False

        #### deps

        - Type: List[String]
        - Value: `List of User Defined Names of projects or services in the projects or services mapping.`
        - Description: "Explicitly list dependencies for this service, any dependency will be built and deployed before this service. Only needed if Torb cannot infer the dependency from templated inputs in the inputs mapping or values mapping."
        - Required: False

### Projects

- Type: Mapping[String, Project]
- Description: "The projects section specifies all projects Torb will configure, initialize, build and deploy to your Kubernetes cluster."
- Required: False

    ### Project

    - Type: Mapping[String, Any]
    - Description: "This is a single project configuration, any number of these can be specified in the projects section of a stack."
    - Required: True

        #### Mapping

        #### project

        - Type: String
        - Value: `Torb Project Name`
        - Description: "The name of the project as defined in an artifact repository."
        - Required: True

        #### values

        - Type: Mapping[String, Any]
        - Value: Any value found in the associated helm chart for this project.
        - Description: "Optional mapping, this can be any value found in the associated helm chart for this project, it will be passed through directly."
        - Required: False

        #### inputs

        - Type: Mapping[String, String | Numeric | Array | Bool]
        - Value: `User Defined`
        - Description: "A mapping of input names to values as defined by the service or project. If values are not supplied Torb will use the defaults set in the service or project by the service or project template maintainer."
        - Required: False

        #### deps

        - Type: List[String]
        - Value: `List of User Defined Names of projects or services in the projects or services mapping.`
        - Description: "Explicitly list dependencies for this project, any dependency will be built and deployed before this service. Only needed if Torb cannot infer the dependency from templated inputs in the inputs mapping or values mapping."
        - Required: False

        #### build

        - Type: Mapping[String, String]
        - Description: "This section defines the configuration for how a docker image will be built for this project. 'tag' and 'registry' are mutually exclusive with 'build_script'.
        - Required: True

            ##### Mapping

            ##### tag

            - Type: String
            - Value: `User Defined`
            - Description: "The tag of the docker image to be built."
            - Required: False

            ##### registry

            - Type: String
            - Value: `User Defined`
            - Description: "The registry this docker image will be pushed to, if set to 'local' this will use Docker's in built registry. Typically this does not play nice with kubernetes and it's recommended to either host your own local registry or use a remote cloud service."
            - Required: False
            
            ##### build_Script

            - Type: String
            - Value: `User Defined`
            - Description: "The path in the project repository to run to build and push the docker image, this is mutually exclusive with 'tag' and 'registry', if you provide this you can't use those."
            - Required: False
