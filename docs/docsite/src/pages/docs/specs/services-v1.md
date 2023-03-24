# Services
## Description
## Spec
### Version

- Type: String
- Value: `Semantic Version`
- Description: "Specifies the version of the Service configuration. This should follow the semantic versioning format."
- Required: True

### Kind

- Type: String
- Value: `service`
- Description: "Specifies what type of configuration this is for Torb. For services, this should be set to 'service'."
- Required: True

### Name

- Type: String
- Value: `Service Name`
- Description: "A unique name identifying the service."
- Required: True

### Inputs

- Type: Mapping[String, String | List]
- Value: `User Defined Value`
- Description: "A mapping of input names to their default values, type, and Helm chart values. These can be overridden by the user during deployment."
- Required: False

### Outputs

- Type: List[String]
- Value: `User Defined Value`
- Description: "A list of output names that the service will generate. These outputs can be used by other services or projects."
- Required: False

### Deploy

- Type: Mapping[String, Any]
- Value: `User Defined Value`
- Description: "A mapping containing deployment-related settings for the Service."
- Required: False

    #### Mapping
    #### Helm

    - Type: Mapping[String, Any]
    - Value: `User Defined Value`
    - Description: "A mapping containing the Helm chart settings for the Service."
    - Required: True or False (depending on whether the service uses Helm)\


        ##### Mapping
        ##### Repository

        - Type: String
        - Value: `Helm Repository URL`
        - Description: "The URL of the Helm repository that contains the chart for this service."
        - Required: True

        ##### Chart

        - Type: String
        - Value: `Helm Chart Name`
        - Description: "The name of the Helm chart to use for this service."
        - Required: True

        ##### Custom

        - Type: Bool
        - Value: `True or False`
        - Description: "If true, the Helm chart is considered a custom chart and can have additional configuration."
        - Required: False

    ##### TF

    - Type: Mapping[String, Any]
    - Value: `User Defined Value`
    - Description: "A mapping containing the Terraform settings for the Service (if applicable)."
    - Required: False (Only needed if using Terraform)

