
use super::*;

#[derive(Debug)]
pub enum UpgradeError {

}

#[derive(Clone)]
pub struct Discovery {
    pub protocols: Arc<Vec<String>>,
}

impl UpgradeInfo for Discovery {
    type Info = String;
    type InfoIter = std::vec::IntoIter<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        (*self.protocols).clone().into_iter()
    }
}

impl<C> InboundUpgrade<C> for Discovery {
    type Output = C;
    type Error = UpgradeError;
    type Future = future::Ready<Result<Self::Output, UpgradeError>>;

    fn upgrade_inbound(self, socket: C, _info: Self::Info) -> Self::Future {
        future::ok(socket)
    }
}

impl<C> OutboundUpgrade<C> for Discovery {
    type Output = C;
    type Error = UpgradeError;
    type Future = future::Ready<Result<Self::Output, UpgradeError>>;

    fn upgrade_outbound(self, socket: C, _info: Self::Info) -> Self::Future {
        future::ok(socket)
    }
}
