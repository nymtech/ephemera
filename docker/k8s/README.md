# Kubernetes

Tested only locally with Docker Desktop for Mac.

## How to setup

### Install kubectl

```bash
brew install kubectl
```

### Most basic configuration

```bash
kubectl create configmap ephemera1 --from-file=config/ephemera1.toml --from-file=config/peers.toml
kubectl create configmap ephemera2 --from-file=config/ephemera2.toml --from-file=config/peers.toml
kubectl create configmap ephemera3 --from-file=config/ephemera3.toml --from-file=config/peers.toml
```

### Create the cluster

```bash
kubectl apply -f ephemera1.deployment.yaml -f ephemera2.deployment.yaml  -f ephemera3.deployment.yaml
kubectl apply -f ephemera1.service.yaml -f ephemera2.service.yaml  -f ephemera3.service.yaml
```

### Check the cluster

```bash
kubectl get pods
kubectl get services
```

### Delete the cluster

```bash
kubectl delete deploy ephemera1-deployment ephemera2-deployment  ephemera3-deployment
kubectl delete svc ephemera1-service ephemera2-service ephemera3-service
```

### Access the cluster

```bash
curl http://127.0.0.1:7000/ephemera/broadcast/blocks/last
```



