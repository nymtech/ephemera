# Metrics Aggregator Example

Simulates Nym-Api metrics collection and rewarding. The goal is to try out how to integrate Ephemera with actual
Nym-Api.

## How it works and what it does

In this example Ephemera doesn't have the central role but is used as a tool for larger goal.

The main actor is the simulated Nym-Api. It works together with simulated Smart Contract.

## Smart Contract

Simulated smart contract has 2 functions:
1. It allows all Nym-Apis to query about current Epoch(used to determine what time to send rewards to Smart Contract).
2. It allows Nym-Apis to submit aggregated rewards for previous Epoch(when current Epoch ends).

## Nym-Api

1. It collects metrics about mixnodes uptimes at regular interval and stores these in its database.
   1. Simulation generates numbers between 0 and 100 for each mixnode.
2. It uses Epoch(queried from Smart Contract at startup) to decide when and what interval to use(Epoch start and end difference)
to calculate average rewards for all mixnodes.
   
### Nym-Api with Ephemera

When Epoch ends, Nym-Api queries average uptime of each mixnode from its database over the previous Epoch duration.
This gives it an array of average updates of each mixnode. It then creates an Ephemera message where payload is
this array. It's done about at the same time by every Nym-Api based on the global Epoch(managed by Smart Contract).

At this point all Nym-Apis are sending their messages to the Ephemera network nodes which will then store these in 
their Ephemera mempool.
After an Ephemera instance is ready to create a block(it uses a trait called Application to determine when to create a block),
it collects these messages from the mempool, puts into block and does its usual reliable broadcast with blocks.

Although each Nym-Api Ephemera instance participates in the Reliable Broadcast for other node blocks,
Nym-Api waits for its locally created block to be finalized. 

When the local block is finalized, Nym-Api collects its messages. 

Each message in the block is from a different Nym-Api and contains list of rewards calculated 
by that Nym-Api for that Epoch. Technically it's a matrix. Rows are single Nym-Api results and columns are mixnodes.

Every Nym-Api then calculates the average(just simple mean for now) of each column. This is the final aggregated rewards for this Nym-Api
instance(from the block it created). 

It then sends this aggregated rewards to the Smart Contract.
Smart Contract accepts only the first submission. It stores the aggregated rewards in its database.

PS! See section below about the difference between current(single) Nym-Api and Nym-Api with Ephemera.

## Reward verification and certification

### Signatures

#### Message signatures
I suggest that we also sign original reward messages(which eventually go into blocks). It would be useful in 2 cases:
1. Ephemera can immediately reject(using Nym-Api custom Application trait) any messages which is not signed correctly.
   2. We can reject messages from unknown network nodes.
   3. This reduces risk of DOS attack and avoids garbage messages in Nym-Api blocks.
   3. It helps to avoid replay attacks
4. It is possible to verify that every message is authentic(who sent it)

#### Block signatures
Block has signatures from all the nodes which participated in its Reliable Broadcast.
It is possible that block contains messages from nodes which didn't participate in its Reliable Broadcast.
It can happen that mempool had messages from those nodes and they were included in the block.

### Verification
Smart Contract has data for each Epoch and corresponding rewards.
Nym-Apis has blocks and messages included in those blocks.

I am not completely aware of the requirements but I imagine that given the reward data in Smart Contract,
anybody can cryptographically verify that final rewards data calculation was carried out by trusted and decentralized network.

Logically we should be able to verify:
1. Which Nym-Api instances signed the block.
2. Which Nym-Api instances sent(and signed) the messages in the block.

## Simulation open issues
* Find best suited average calculation algorithm which takes into account outliers and missing values.
  * It can even give different weights to different Nym-Apis based on their past reliability.
* Nym-Apis need to find out which one from the cluster was successful in submitting the aggregated rewards to the Smart Contract.
  * The simplest solution would be to store it in Smart Contract.

## Current Nym-Api(simulated)

* Runs a **metrics collector** which simulates metrics collection from mixnodes.
    * Generates random metrics for each mixnode and saves it in database.
* Runs a **reward distributor** which simulates reward distribution.
    * Aggregates the metrics collected by the **metrics collector** from the database.
    * Sends the aggregated metrics to the **"smart contract"**.
* Runs a **"smart contract"**(just a http server) which listens for reward distribution requests.
    * It just stores the aggregated metrics in a database for introspection.

## **TODO** Nym-Api with Ephemera(simulated)

* Runs a **metrics collector** which simulates metrics collection from mixnodes.
    * Generates random metrics for each mixnode and saves them in a database.
* Runs a **reward distributor** which simulates reward distribution.
    * Aggregates the metrics collected by the **metrics collector** from the database.
    * **--------------------------------DIFFERENCE----------------------------------------------**
    * **Uses Ephemera to distribute the local aggregated metrics to other Ephemera nodes**
    * **After the local Ephemera node have finalized local block with the aggregated metrics from all nodes,
      calculates the summary of the aggregated metrics and tries to send it to the "smart contract".**
    * **"Smart contract" accepts only the first request(all nodes race to submit it)**
    * **--------------------------------END OF DIFFERENCE--------------------------------**
* Runs a **"smart contract"**(just a http server) which listens for reward distribution requests.
    * It just stores the aggregated metrics in a database for introspection.




## How to run

See [README.md](../../scripts/README.md) in the root of the repository.