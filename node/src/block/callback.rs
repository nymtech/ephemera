use crate::block::SignedMessage;

pub(crate) trait BlockProducerCallback {
    fn on_proposed_messages(&self, messages: &[SignedMessage]) -> anyhow::Result<bool>;
}

pub(crate) struct DummyBlockProducerCallback;

impl BlockProducerCallback for DummyBlockProducerCallback {
    fn on_proposed_messages(&self, _: &[SignedMessage]) -> anyhow::Result<bool> {
        Ok(true)
    }
}

pub(crate) struct MinMessageCountBlockProducerCallback(pub(crate) usize);

impl BlockProducerCallback for MinMessageCountBlockProducerCallback {
    fn on_proposed_messages(&self, messages: &[SignedMessage]) -> anyhow::Result<bool> {
        if messages.len() >= self.0 {
            return Ok(true);
        }
        Ok(false)
    }
}
