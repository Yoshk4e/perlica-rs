use crate::net::NetContext;
use perlica_logic::mail::StoredMail;
use perlica_proto::{
    CsDeleteAllMail, CsDeleteMail, CsGetAllMailAttachment, CsGetMail, CsGetMailAttachment,
    CsMailDef, CsReadMail, MailContent, RewardItem, ScDelMailNotify, ScGetMail,
    ScGetMailAttachment, ScNewMailNotify, ScReadMail, ScSyncAllMail,
};
use tracing::{debug, warn};

pub async fn push_mail_sync(ctx: &mut NetContext<'_>) -> bool {
    let mail_id_list = ctx.player.mail.all_ids();
    let new_mail_tag = ctx.player.mail.has_unread();

    debug!(
        "Pushing mail sync: uid={}, count={}, unread={}",
        ctx.player.uid,
        mail_id_list.len(),
        new_mail_tag
    );

    ctx.notify(ScSyncAllMail {
        mail_id_list,
        new_mail_tag,
    })
    .await
    .is_ok()
}

pub async fn deliver_login_mails(ctx: &mut NetContext<'_>, is_new_player: bool) {
    let mail = if is_new_player {
        StoredMail::make_welcome_mail()
    } else {
        StoredMail::make_login_greeting_mail()
    };

    let new_id = ctx.player.mail.add_mail(mail);
    debug!(
        "Delivered {} mail: uid={}, mail_id={}",
        if is_new_player { "welcome" } else { "greeting" },
        ctx.player.uid,
        new_id
    );

    if let Err(e) = ctx
        .notify(ScNewMailNotify {
            mail_id_list: vec![new_id],
        })
        .await
    {
        warn!(
            "Failed to notify new mail: uid={}, error={}",
            ctx.player.uid, e
        );
    }
}

pub async fn on_cs_get_mail(ctx: &mut NetContext<'_>, req: CsGetMail) -> ScGetMail {
    debug!(
        "CsGetMail: uid={}, ids={:?}",
        ctx.player.uid, req.mail_id_list
    );

    let mails = ctx.player.mail.get_by_ids(&req.mail_id_list);
    let mail_list = mails.iter().map(|m| build_cs_mail_def(m)).collect();

    ScGetMail {
        mail_list,
        has_extra_attachment_item: false,
    }
}

pub async fn on_cs_read_mail(ctx: &mut NetContext<'_>, req: CsReadMail) -> ScReadMail {
    debug!(
        "CsReadMail: uid={}, mail_id={}",
        ctx.player.uid, req.mail_id
    );

    if !ctx.player.mail.mark_read(req.mail_id) {
        warn!(
            "CsReadMail: mail not found: uid={}, mail_id={}",
            ctx.player.uid, req.mail_id
        );
    }

    ScReadMail {
        mail_id: req.mail_id,
    }
}

pub async fn on_cs_delete_mail(ctx: &mut NetContext<'_>, req: CsDeleteMail) -> ScDelMailNotify {
    debug!(
        "CsDeleteMail: uid={}, mail_id={}",
        ctx.player.uid, req.mail_id
    );

    if !ctx.player.mail.delete_mail(req.mail_id) {
        warn!(
            "CsDeleteMail: mail not found: uid={}, mail_id={}",
            ctx.player.uid, req.mail_id
        );
    }

    ScDelMailNotify {
        mail_id_list: vec![req.mail_id],
    }
}

pub async fn on_cs_delete_all_mail(
    ctx: &mut NetContext<'_>,
    req: CsDeleteAllMail,
) -> ScDelMailNotify {
    debug!(
        "CsDeleteAllMail: uid={}, types={:?}",
        ctx.player.uid, req.mail_type_list
    );

    let deleted = ctx.player.mail.delete_by_types(&req.mail_type_list);
    debug!(
        "CsDeleteAllMail: deleted {} mails: uid={}",
        deleted.len(),
        ctx.player.uid
    );

    ScDelMailNotify {
        mail_id_list: deleted,
    }
}

pub async fn on_cs_get_mail_attachment(
    ctx: &mut NetContext<'_>,
    req: CsGetMailAttachment,
) -> ScGetMailAttachment {
    debug!(
        "CsGetMailAttachment: uid={}, mail_id={}",
        ctx.player.uid, req.mail_id
    );

    match ctx.player.mail.claim_attachment(req.mail_id) {
        Some(items) => {
            debug!(
                "CsGetMailAttachment: claimed {} items from mail {}: uid={}",
                items.len(),
                req.mail_id,
                ctx.player.uid
            );
            // TODO: actually grant items to player inventory here.
            ScGetMailAttachment {
                success_mail_id_list: vec![req.mail_id],
                failed_mail_id_list: vec![],
            }
        }
        None => {
            warn!(
                "CsGetMailAttachment: mail not found or already claimed: uid={}, mail_id={}",
                ctx.player.uid, req.mail_id
            );
            ScGetMailAttachment {
                success_mail_id_list: vec![],
                failed_mail_id_list: vec![req.mail_id],
            }
        }
    }
}

pub async fn on_cs_get_all_mail_attachment(
    ctx: &mut NetContext<'_>,
    req: CsGetAllMailAttachment,
) -> ScGetMailAttachment {
    debug!(
        "CsGetAllMailAttachment: uid={}, types={:?}",
        ctx.player.uid, req.mail_type_list
    );

    let (success, failed) = ctx.player.mail.claim_all_attachments(&req.mail_type_list);

    debug!(
        "CsGetAllMailAttachment: claimed={}, failed={}: uid={}",
        success.len(),
        failed.len(),
        ctx.player.uid
    );

    // TODO: grant items for each success ID.
    ScGetMailAttachment {
        success_mail_id_list: success,
        failed_mail_id_list: failed,
    }
}

fn build_cs_mail_def(m: &perlica_logic::mail::StoredMail) -> CsMailDef {
    let expire_time = if m.expire_time < 0 { 0 } else { m.expire_time };

    let item_list = m
        .items
        .iter()
        .map(|(id, count)| RewardItem {
            id: id.clone(),
            count: *count,
            inst: None,
        })
        .collect();

    CsMailDef {
        mail_type: m.mail_type,
        mail_id: m.mail_id,
        expire_time,
        is_read: m.is_read,
        is_attachment_got: m.is_attachment_got,
        send_time: m.send_time,
        mail_content: Some(MailContent {
            template_id: m.template_id.clone(),
            title: m.title.clone(),
            content: m.content.clone(),
            sender_name: m.sender_name.clone(),
            sender_icon: m.sender_icon.clone(),
            params: Default::default(),
        }),
        item_list,
    }
}
