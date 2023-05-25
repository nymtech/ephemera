# Kubernetes

## AWS

[eksctl](https://eksctl.io/) is the official CLI for Amazon EKS

```bash
brew install eksctl
```

### Create AWS EKS cluster with `eksctl`

Replace/change/add values in `./k8s/eks/ephemera-cluster-create.yaml`.

```bash
eksctl create cluster -f ./eks/ephemera-cluster-create.yaml --version=1.26
```

### Install kubectl

```bash
brew install kubectl
```

### Configure kubectl

```bash
aws eks --region us-east-1 update-kubeconfig --name ephemera
```

## If needed, create the ECR repository for local images

```bash
aws ecr create-repository --repository-name nym --region us-east-1
```

### Docker login to AWS ECR

```bash
aws ecr get-login-password --region us-east-1 | docker login --username AWS --password-stdin <ACCOUNT_ID>.dkr.ecr.us-east-1.amazonaws.com
```

## Deploy Ephemera to Kubernetes

### Build the Docker image

```bash
cd ../../node
docker  build . -f ../docker/k8s/Dockerfile -t ephemera:latest --platform linux/amd64
```

### Tag the Docker image

```bash
docker tag ephemera:latest 526189391121.dkr.ecr.us-east-1.amazonaws.com/nym:ephemera
```

### Push the Docker image

```bash
docker push <ACCOUNT_ID>.dkr.ecr.us-east-1.amazonaws.com/nym:ephemera
```

### Create Ephemera cluster

```bash
kubectl create configmap ephemera1 --from-file=config/ephemera1.toml --from-file=config/peers.toml
kubectl create configmap ephemera2 --from-file=config/ephemera2.toml --from-file=config/peers.toml
kubectl create configmap ephemera3 --from-file=config/ephemera3.toml --from-file=config/peers.toml
```
    
```bash
kubectl apply -f ephemera1.all.yaml -f ephemera2.all.yaml  -f ephemera3.all.yaml
```

### Check Ephemera cluster

```bash
kubectl get pods
kubectl get services
```

### Delete Ephemera cluster

```bash
kubectl delete deploy ephemera1-deployment ephemera2-deployment  ephemera3-deployment
kubectl delete svc ephemera1-svc ephemera2-svc ephemera3-svc
```

### Delete Ephemera cluster configmaps

```bash
kubectl delete configmap ephemera1 ephemera2 ephemera3
```



