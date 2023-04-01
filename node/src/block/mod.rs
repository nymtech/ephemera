//! # Block Manager

//! In blockchain technology blocks have two main purposes:
//! 1. To maintain chain of blocks, so that the validity of a each block can be cryptographically
//!    verified by the previous chain of blocks.
//! 2. As a unit of consensus, each block contains a set of transactions/messages/actions that are agreed upon by
//!    the network. This set of transactions is chosen from the global set of all possible transactions that are pending.
//!    We call the set of transactions in a block consensus because the set of nodes trying to achieve global shared state
//!    agreed on this particular set of transactions.
//!
//! Ephemera is not a blockchain. But it uses blocks to agree on the set of transactions in a block.
//! But at the same time it doesn't behave like a blockchain consensus algorithm.
//! We may say that it allows each application that uses Ephemera to "propose" something what can be
//! afterwards to used to achieve consensus.

//! # In Summary
//!
//! A) Ephemera provides functionality to reach agreement on a single value between a set of nodes.
//! B) Ephemera also provides the concept of a block, which application can take benefit to reach consensus.
//!
//! ## Reliable broadcast, consensus and blocks
//!
//! In distributed systems(including byzantine), we try to solve the the general of problem of reaching to a commons state
//! between a set of nodes.
//!
//! Usually it's defined using the following properties:
//! 1.1 Agreement: All nodes agree on the same value.(TODO clarify)
//! 1.2 Consensus: All nodes agree on the same value.(TODO clarify)
//! 2. Validity: All nodes agree on a value that is valid.
//! 3. Termination: All nodes eventually agree on a value.
//!
//! Reliable broadcast ensures the properties of 1.1 and 2.
//! It's left to a particular consensus algorithm to ensure the termination property.
//!
//! The most useful feature of consensus in blockchain is that it guarantees total ordering of transactions.
//! Reliable broadcast helps to ensure this total ordering.
//!
//! # Ephemera specific properties
//!
//! Because Ephemera doesn't use the idea of leader, we can say that it solves consensus partially.
//! It allows each instance to create a block. And then it's up to an application to decide which block to use.
//!
//! Also as it doesn't implement a full consensus algorithm, it doesn't need to ensure the termination property.
//! There's no algorithm in place what tries to reach a consensus about a single block globally and sequentially
//! in time.
//!
//! When a block contains a single message, then it's semantically equivalent to a reliable broadcast.
//!
//! But when a block contains multiple messages, then it can be part of a consensus process. Except that in Ephemera each node
//! can create a block. To achieve consensus in a more traditional sense, it needs an application help if more strict
//! consensus is required.
//!
//! For example, Nym-Api allows each node to create a block but uses external coordinator(a smart contract)
//! to decide which block to use.
//!
//! # The Block manager
//!
//! Block manager is quite simple. It keeps pending messages in memory and puts all of them into a block
//! at predefined intervals. That's all it does.
//!
//! If the block actually will be broadcast or not is decided by the application. If not, it will produce next block with
//! the same messages plus the new ones.
//!
//! When application shuts down, pending messages are lost.
//!
//! When a block gets accepted by reliable broadcast then Block Manager will remove all messages included in the block from the
//! pending messages queue.
//!
//! # Synchronization and duplicate messages in sequence of blocks
//!
//! When previous block hasn't been accepted yet, then the next block will contain the same messages as the previous one.
//! One way to solve this is that an application itself keeps track of duplicate messages and discards them if necessary.
//!
//! But it seems a reasonable assumption that in general duplicate messages are unwanted. Therefore, Ephemera solves this
//! by dropping previous blocks which get Finalised/Committed after a new block has been created.

pub(crate) mod builder;
pub(crate) mod manager;
pub(crate) mod message_pool;
pub(crate) mod producer;
pub(crate) mod types;
