
use super::*;

pub struct ArcConfig {
    pub inner: Arc<Config>,
}

impl From<&Arc<Config>> for ArcConfig {
    fn from(inner: &Arc<Config>) -> Self {
        ArcConfig {
            inner: Arc::clone(&inner),
        }
    }
}

#[derive(Debug)]
pub enum UpgradeError {

}

impl UpgradeInfo for ArcConfig {
    type Info = String;
    type InfoIter = std::vec::IntoIter<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        self.inner.protocols.clone().into_iter()
    }
}

impl<C> InboundUpgrade<C> for ArcConfig {
    type Output = C;
    type Error = UpgradeError;
    type Future = future::Ready<Result<Self::Output, UpgradeError>>;

    fn upgrade_inbound(self, socket: C, _info: Self::Info) -> Self::Future {
        future::ok(socket)
    }
}

impl<C> OutboundUpgrade<C> for ArcConfig {
    type Output = C;
    type Error = UpgradeError;
    type Future = future::Ready<Result<Self::Output, UpgradeError>>;

    fn upgrade_outbound(self, socket: C, _info: Self::Info) -> Self::Future {
        future::ok(socket)
    }
}
