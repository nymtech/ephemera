use crate::block::SignedMessage;

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct ApplicationInfo<'a> {
    pub(crate) label: Option<String>,
    pub(crate) accepted_messages: &'a [SignedMessage],
}

impl<'a> ApplicationInfo<'a> {
    pub(crate) fn new(label: Option<String>, messages: &'a [SignedMessage]) -> Self {
        Self {
            label,
            accepted_messages: messages,
        }
    }
}

pub(crate) trait BlockProducerCallback {
    fn on_proposed_block<'a>(
        &self,
        messages: &'a [SignedMessage],
    ) -> anyhow::Result<Option<ApplicationInfo<'a>>>;
}

pub(crate) struct DummyBlockProducerCallback;

impl BlockProducerCallback for DummyBlockProducerCallback {
    fn on_proposed_block<'a>(
        &self,
        messages: &'a [SignedMessage],
    ) -> anyhow::Result<Option<ApplicationInfo<'a>>> {
        let accepted = ApplicationInfo::new(None, messages);
        Ok(Some(accepted))
    }
}

pub(crate) struct MinMessageCountBlockProducerCallback(pub(crate) usize);

impl BlockProducerCallback for MinMessageCountBlockProducerCallback {
    fn on_proposed_block<'a>(
        &self,
        messages: &'a [SignedMessage],
    ) -> anyhow::Result<Option<ApplicationInfo<'a>>> {
        if messages.len() >= self.0 {
            let accepted = ApplicationInfo::new(None, messages);
            return Ok(Some(accepted));
        }
        Ok(None)
    }
}
