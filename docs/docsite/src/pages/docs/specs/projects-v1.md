# Projects

## Description

## Spec

### Version
- Type: String
- Value: Semantic Version
- Description: "The version of the Project configuration. This should follow the semantic versioning format."
- Required: True
### Kind
- Type: String
- Value: project
- Description: "Specifies that this configuration is for a Project. This should be set to 'project'."
- Required: True
### Name
- Type: String
- Value: User Defined Name
- Description: "A human-readable name for the Project. This helps to identify the Project and its purpose."
- Required: True
### Lang
- Type: String
- Value: Programming Language
- Description: "The programming language used for the Project."
- Required: True
### Inputs
- Type: Mapping[String, Tuple[Bool | Numeric | String | Array, Any, String]]
- Value: User Defined Value
- Description: "A mapping of input names to a tuple containing the input's type, default value, and optional mapping to a value in the helm chart. Four types are supported, Bool, Numeric, String, and Array. Nested Arrays are not supported at this time."
- Required: False
### Files
- Type: List[String]
- Value: List of File Names
- Description: "A list of files needed for the Project, such as Dockerfiles, templates, and other required files."
- Required: False
### Init
- Type: List[String]
- Value: List of Bash Commands
- Description: "A list of Bash commands that are executed during the initialization phase of the Project. These commands are responsible for setting up the Project environment, creating necessary files and folders, and performing any other required setup tasks."
- Required: False
### Build
- Type: Mapping[String, String]
- Value: User Defined Value
- Description: "A mapping containing build-related settings for the Project."
- Required: False

    #### Mapping
    #### script_path
    - Type: String
    - Value: Path to Build Script
    - Description: "The path to the build script file that will be executed during the build process. Leave empty if using Dockerfile."
    - Required: False
    #### dockerfile
    - Type: String
    - Value: Dockerfile Name
    - Description: "The name of the Dockerfile that will be used to build the Docker image."
    - Required: True
    #### registry
    - Type: String
    - Value: Registry URL
    - Description: "The URL of the container registry where the Docker image will be pushed."
    - Required: True
    #### tag
    - Type: String
    - Value: User Defined Tag
    - Description: "The tag for the Docker image that will be built and pushed to the container registry."
    - Required: True
    #### Deploy
    - Type: Mapping[String, Any]
    - Value: User Defined Value
    - Description: "A mapping containing deployment-related settings for the Project."
    - Required: False

        ##### Mapping
        ##### helm
        - Type: Mapping[String, Any]
        - Value: User Defined Value
        - Description: "A mapping containing Helm-related settings for deploying the Project to a Kubernetes cluster."
        - Required: False

            ###### Mapping
            ###### custom
            - Type: Bool
            - Value: True | False
            - Description: "Indicates whether a custom Helm chart is used for deployment. Set to 'true' for custom charts, and 'false' for using pre-existing charts."
            - Required: True
            ###### chart
            - Type: String
            - Value: Helm Chart Name
            - Description: "The name of the Helm chart used for deploying the Project. Provide the full path for custom charts, and the name for pre-existing charts."
            - Required: True
            ###### chart_version
            - Type: String
            - Value: Semantic Version
            - Description: "The version of the Helm chart used for deployment. This should follow the semantic versioning format."
            - Required: True
            ###### repository
            - Type: String
            - Value: Helm Repository URL
            - Description: "The URL of the Helm repository hosting the chart, if using a pre-existing chart. Leave empty if using a custom chart."
            - Required: False
            ###### tf
            - Type: Mapping[String, Any] | Null
            - Value: `User Defined Value`
            - Description: "A mapping containing Terraform-related settings for deploying the Project. Currently not specified in the examples provided."
            - Required: False