## Torb Specification Version 1.0

Torb is comprised of 3 layers, layer one, the Stack layer is the higest level. This is what end users will typically interact with. This is the layer that combines different services and projects into a coherent development stack that Torb can then deploy onto a configured Kubernetes cluster.

Layer two is the layer comprised of invidivdual services and projects. Services are things like Postgresql and Traefik which are tools developers depend on for their projects, but that they are not (usually) writing code for. That is the biggest distinction between a Torb Service and a Torb Project. Services additionally do not have build or initialization steps. Projects however are where developers will actually write code and require the most amount of flexibility and customization both for end users and those who are contributing to Torb.

Layer three is the final layer and is made up of helm charts, both exisitng open source ones and ones created specifically for Torb, as well as the Terraform code used to actually deploy the charts after everything is configured and resolved by Torb. This layer is not covered here.

Stacks, Services and Projects are all configured by a small YAML based configuration DSL (domain specific language) and are made of a bunch of different fields and values. This documentation covers the 1.0 specfication of this configuration language and details each possible configuration in detail.

Specifications:
- [Stacks V1 Specification](./specs/stacks-v1)
- [Services V1 Specification](./specs/services-v1)
- [Projects V1 Specification](./specs/projects-v1)
