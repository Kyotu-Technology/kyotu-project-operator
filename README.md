# Kyotu Project Operator

## Overview

This is a Kubernetes operator for the Kyotu Project. It is written in Rust and uses kube-rs.
It manages the lifecycle of the Kyotu Project, creates repositories, and manages the namespaces, and add ArgoCD applications.

## What it does?

When crd is created it does the following:

- Creates a namespace for the Kyotu Project. If the namespace already exists it will not be created.
- Creates a Gitlab group for the Kyotu Project. If the group already exists it will not be created.
- Creates a Group Access Token for the Kyotu Project with access to docker registry. IF token already exists it will be rotated.
- Creates kubernetes pull secret for the Kyotu Project using the Gitlab Group Access Token
- Creates argocd application for the Kyotu Project by adding application to deployment repository
- Creates rbacs for argocd and vault and checks them out to the flux repository

When crd is deleted it does the following:

- Deletes the namespace for the Kyotu Project. If the nasmepace existed before it will not be deleted.
- It does not delete the Gitlab group or the repositories.
- Deletes Group Access Token for the Kyotu Project.
- Deletes kubernetes pull secret for the Kyotu Project.
- Deletes argocd application for the Kyotu Project by removing application from deployment repository
- Deletes rbacs for argocd and vault and checks them out to the flux repository

## How to use it?

### Install the operator

```bash
aws ecr get-login-password | helm registry login --username AWS --password-stdin 480102916536.dkr.ecr.us-east-1.amazonaws.com
helm install operator oci://480102916536.dkr.ecr.us-east-1.amazonaws.com/kyotu-project-operator --version 1.1.6 -n <namespace>
```

### Configuration options

Chart is configures using values.yaml file. Here are the options:
| Parameter | Description | Default |
| --------- | ----------- | ------- |
| `config.gitlabUrl` | URL to gitlab | `https://gitlab.k8s.kyotutechnology.com` |
| `config.argoRepo` | Deployment repo address for cloning and pushing| `https://operator@gitlab.k8s.kyotutechnology.com/operations/deployment.git`|
| `config.fluxRepo` |Flux repo address for clonning and pushing| `git@github.com:Kyotu-Technology/aws-k8s-flux.git`|
| `config.repoBranch` | Branch where changes will be pushed | `test`|
| `config.argo.deployKeySecret` | Secret name storing token for Deployment repo| `kyotu-project-operator-token`|
| `config.argo.deployKeySecretKey` | Secret key where token is saved | `deployKey`|
| `config.flux.deployKeySecret` | Secret name storing token for Flux repo| `kyotu-project-operator-token`|
| `config.flux.deployKeySecretKey` | Secret key where token is saved | `deployKey`|
| `config.gitlab.tokenSecret` | Secret name for Token that has access to Gitlab API| `kyotu-project-operator-token`|
| `config.gitlab.tokenSecretKey` | Secret key where token is saved | `gitlabToken`|
| `config.logLevel` |Log level configuration| `debug`|

### Create a Kyotu Project

```bash
kubectl apply -f ./manifests/project_example.yaml
``` 

### Delete a Kyotu Project

```bash
kubectl delete -f ./manifests/project_example.yaml
```

## Example CRD

```yaml
apiVersion: kyotu.tech/v1
kind: Project
metadata:
  name: test-project
spec:
  projectId: test-project
  environmentType: dev
  googleGroup: test.crew@kyotutechnology.com
```

## To Do

- [ ] Add multiple environments
- [ ] Add status to crd
- [ ] Add metrics