use crate::{GossipsubEvent, GossipsubEventId};
use bcr_ebill_core::{
    bill::{BillKeys, RecourseReason},
    blockchain::bill::{BillBlock, BillBlockchain},
    contact::IdentityPublicData,
    identity::Identity,
    notification::ActionType,
};
use borsh::to_vec;
use log::info;

use super::{BillAction, Result, service::BillService};

impl BillService {
    pub(super) async fn notify_for_block_action(
        &self,
        blockchain: &BillBlockchain,
        bill_keys: &BillKeys,
        bill_action: &BillAction,
        identity: &Identity,
    ) -> Result<()> {
        let last_version_bill = self
            .get_last_version_bill(blockchain, bill_keys, identity)
            .await?;

        // calculate possible recipients
        let mut recipients = vec![];
        if matches!(
            bill_action,
            BillAction::RejectAcceptance
                | BillAction::RejectBuying
                | BillAction::RejectPayment
                | BillAction::RejectPaymentForRecourse
        ) {
            if let Some(self_identity) = IdentityPublicData::new(identity.clone()) {
                recipients.push(self_identity);
            }
            for node_id in blockchain.get_all_nodes_from_bill(bill_keys)? {
                if let Some(contact) = self.contact_store.get(&node_id).await?.map(|c| c.into()) {
                    recipients.push(contact);
                }
            }
        };

        match bill_action {
            BillAction::Accept => {
                self.notification_service
                    .send_bill_is_accepted_event(&last_version_bill)
                    .await?;
            }
            BillAction::RequestAcceptance => {
                self.notification_service
                    .send_request_to_accept_event(&last_version_bill)
                    .await?;
            }
            BillAction::RequestToPay(_) => {
                self.notification_service
                    .send_request_to_pay_event(&last_version_bill)
                    .await?;
            }
            BillAction::RequestRecourse(recoursee, recourse_reason) => {
                let action_type = match recourse_reason {
                    RecourseReason::Accept => ActionType::AcceptBill,
                    RecourseReason::Pay(_, _) => ActionType::PayBill,
                };
                self.notification_service
                    .send_recourse_action_event(
                        &last_version_bill.id,
                        Some(last_version_bill.sum),
                        action_type,
                        recoursee,
                    )
                    .await?;
            }
            BillAction::Recourse(recoursee, sum, _) => {
                self.notification_service
                    .send_bill_recourse_paid_event(&last_version_bill.id, Some(*sum), recoursee)
                    .await?;
            }
            BillAction::Mint(_, _, _) => {
                self.notification_service
                    .send_request_to_mint_event(&last_version_bill)
                    .await?;
            }
            BillAction::OfferToSell(buyer, sum, _) => {
                self.notification_service
                    .send_offer_to_sell_event(&last_version_bill.id, Some(*sum), buyer)
                    .await?;
            }
            BillAction::Sell(buyer, sum, _, _) => {
                self.notification_service
                    .send_bill_is_sold_event(&last_version_bill.id, Some(*sum), buyer)
                    .await?;
            }
            BillAction::Endorse(_) => {
                self.notification_service
                    .send_bill_is_endorsed_event(&last_version_bill)
                    .await?;
            }
            BillAction::RejectAcceptance => {
                self.notification_service
                    .send_request_to_action_rejected_event(
                        &last_version_bill.id,
                        Some(last_version_bill.sum),
                        ActionType::AcceptBill,
                        recipients,
                    )
                    .await?;
            }
            BillAction::RejectBuying => {
                self.notification_service
                    .send_request_to_action_rejected_event(
                        &last_version_bill.id,
                        Some(last_version_bill.sum),
                        ActionType::BuyBill,
                        recipients,
                    )
                    .await?;
            }
            BillAction::RejectPayment => {
                self.notification_service
                    .send_request_to_action_rejected_event(
                        &last_version_bill.id,
                        Some(last_version_bill.sum),
                        ActionType::PayBill,
                        recipients,
                    )
                    .await?;
            }
            BillAction::RejectPaymentForRecourse => {
                self.notification_service
                    .send_request_to_action_rejected_event(
                        &last_version_bill.id,
                        Some(last_version_bill.sum),
                        ActionType::RecourseBill,
                        recipients,
                    )
                    .await?;
            }
        };
        Ok(())
    }

    pub(super) async fn propagate_block(&self, bill_id: &str, block: &BillBlock) -> Result<()> {
        let block_bytes = to_vec(block)?;
        let event = GossipsubEvent::new(GossipsubEventId::BillBlock, block_bytes);
        let message = event.to_byte_array()?;

        self.client
            .clone()
            .add_message_to_bill_topic(message, bill_id)
            .await?;
        Ok(())
    }

    pub(super) async fn propagate_bill_for_node(&self, bill_id: &str, node_id: &str) -> Result<()> {
        self.client
            .clone()
            .add_bill_to_dht_for_node(bill_id, node_id)
            .await?;
        Ok(())
    }

    pub(super) async fn propagate_bill(
        &self,
        bill_id: &str,
        drawer_node_id: &str,
        drawee_node_id: &str,
        payee_node_id: &str,
    ) -> Result<()> {
        let mut client = self.client.clone();

        for node in [drawer_node_id, drawee_node_id, payee_node_id] {
            if !node.is_empty() {
                info!("issue bill: add {} for node {}", bill_id, &node);
                client.add_bill_to_dht_for_node(bill_id, node).await?;
            }
        }

        client.subscribe_to_bill_topic(bill_id).await?;
        client.start_providing_bill(bill_id).await?;
        Ok(())
    }
}
