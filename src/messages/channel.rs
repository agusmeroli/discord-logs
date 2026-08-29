use serenity::all::{
    AuditLogEntry, Change, Channel, ChannelAction, ChannelFlags, ChannelId, ChannelOverwriteAction,
    ChannelType, Colour, Context, CreateEmbed, CreateMessage, EntityType, PermissionOverwrite,
    PermissionOverwriteType, Permissions, ThreadAction, User, UserId, audit_log::Action,
};

use crate::{
    find_change, format_boolean_change, format_numeric_change, format_numeric_change_operation,
    format_string_change,
    messages::utils::{build_embed_author, format_channel, format_user},
    unwrap_change,
};

pub async fn build_channel_message(
    entry: AuditLogEntry,
    user: Option<User>,
    ctx: &Context,
) -> Option<CreateMessage> {
    let Some(target_id) = entry.target_id else {
        log::error!("No target channel id provided");
        return None;
    };

    let user_str = format_user(&user, entry.user_id);

    let channel_id = ChannelId::new(target_id.get());
    let channel = channel_id.to_channel(&ctx).await.ok();
    let channel_str = format_channel(&channel, channel_id);

    let channel_type = if let Some(Channel::Guild(gc)) = channel {
        match gc.kind {
            ChannelType::PublicThread => "thread",
            ChannelType::PrivateThread => "private thread",
            ChannelType::Forum => "forum",
            ChannelType::Voice => "voice channel",
            _ => "channel",
        }
    } else {
        "channel"
    };

    let (action, colour) = match entry.action {
        Action::Channel(ChannelAction::Create) | Action::Thread(ThreadAction::Create) => {
            ("created", Colour::new(0x00FF00))
        }
        Action::Channel(ChannelAction::Delete) | Action::Thread(ThreadAction::Delete) => {
            ("deleted", Colour::new(0xFF0000))
        }
        Action::Channel(ChannelAction::Update) | Action::Thread(ThreadAction::Update) => {
            // ignore channel updates made by bots
            if let Some(user) = &user
                && user.bot
            {
                return None;
            }
            ("updated", Colour::new(0xFFAA00))
        }
        a => {
            log::error!(
                "Invalid action passed to channel message builder: {}",
                a.num()
            );
            ("unknown action", Colour::new(0x000000))
        }
    };

    let changes = if let Some(changes) = entry.changes {
        changes
            .iter()
            .filter_map(build_channel_change_line)
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        String::new()
    };

    let embed_author = build_embed_author(&user, entry.user_id);
    let message = format!("{user_str} **{action} {channel_type}** {channel_str}\n\n{changes}");
    let title = format!("{channel_type} {action}").to_uppercase();

    let embed = CreateEmbed::new()
        .title(title)
        .author(embed_author)
        .color(colour)
        .description(message);

    Some(CreateMessage::new().embed(embed))
}

pub async fn build_permission_override_message(
    entry: AuditLogEntry,
    user: Option<User>,
    ctx: &Context,
) -> Option<CreateMessage> {
    let Some(target_id) = entry.target_id else {
        log::error!("No target channel id provided");
        return None;
    };

    let Some(options) = entry.options else {
        return None;
    };

    let Some(permission_target_id) = options.id else {
        return None;
    };

    let user_str = format_user(&user, entry.user_id);

    let channel_id = ChannelId::new(target_id.get());
    let channel = channel_id.to_channel(&ctx).await.ok();
    let channel = format_channel(&channel, channel_id);

    let permission_target_string = if let Some(role_name) = options.role_name {
        if role_name == "@everyone" {
            "- **Permissions for** @everyone".to_string()
        } else {
            format!("- **Permissions for role** <@&{permission_target_id}>({role_name})")
        }
    } else {
        let user_id = UserId::new(permission_target_id.get());
        let user = user_id.to_user(&ctx).await.ok();
        format!("- **Permissions for user** {}", format_user(&user, user_id))
    };

    let Some(changes) = entry.changes else {
        log::error!("Changes not present in permission override log");
        return None;
    };

    let (action, colour) = match entry.action {
        Action::ChannelOverwrite(ChannelOverwriteAction::Create) => {
            ("created", Colour::new(0x00FF00))
        }
        Action::ChannelOverwrite(ChannelOverwriteAction::Delete) => {
            ("deleted", Colour::new(0xFF0000))
        }
        Action::ChannelOverwrite(ChannelOverwriteAction::Update) => {
            ("updated", Colour::new(0xFFAA00))
        }
        a => {
            log::error!(
                "Invalid action passed to channel message builder: {}",
                a.num()
            );
            ("unknown action", Colour::new(0x000000))
        }
    };

    let permission_changes_str = unwrap_changes(&changes);

    let embed_author = build_embed_author(&user, entry.user_id);
    let message = format!(
        "{user_str} **{action} permission override**\n\
                    **for channel** {channel}\n\n\
                    {permission_target_string}:\n\
                    {permission_changes_str}"
    );
    let title = format!("PERMISSION OVERRIDE {}", action.to_uppercase());

    let embed = CreateEmbed::new()
        .title(title)
        .author(embed_author)
        .color(colour)
        .description(message);

    Some(CreateMessage::new().embed(embed))
}

fn build_channel_change_line(change: &Change) -> Option<String> {
    Some(match change {
        Change::UserLimit { old, new } => format_numeric_change!("User limit", "", old, new),
        Change::RateLimitPerUser { old, new } => format_numeric_change!("Slowmode", "s", old, new),
        Change::Name { old, new } => format_string_change!("Name", old, new),
        Change::Topic { old, new } => format_string_change!("Description", old, new),
        Change::Nsfw { old, new } => format_boolean_change!("NSFW", old, new),
        Change::Locked { old, new } => format_boolean_change!("Locked", old, new),
        Change::Archived { old, new } => format_boolean_change!("Archived", old, new),
        Change::Invitable { old, new } => format_boolean_change!("Inviteable", old, new),

        Change::DefaultAutoArchiveDuration { old, new } => {
            format_numeric_change_operation!("Archive duration", "h", old, new, |v| v / 60)
        }
        Change::Bitrate { old, new } => {
            format_numeric_change_operation!("Bitrate", "kbps", old, new, |v| v / 1000)
        }

        Change::Type { old, new } => match (old, new) {
            (Some(old), Some(new)) => format!(
                "- **Type:** `{}` ➜ `{}`",
                format_channel_type(old),
                format_channel_type(new)
            ),
            (_, Some(new)) => format!("- **Type:** `{}`", format_channel_type(new)),
            _ => return None,
        },

        // TODO
        Change::PermissionOverwrites { old, new } => match (old, new) {
            (_, Some(new)) => format_access_permission(new),
            (Some(old), _) => format_access_permission(old),
            _ => return None,
        },
        Change::Flags { old, new } => match (old, new) {
            (Some(old), Some(new)) => return format_flags_diff(old, new),
            (None, Some(new)) => return format_flags(new),
            (Some(old), None) => return format_flags(old),
            _ => return None,
        },

        _ => return None,
    })
}

fn format_channel_type(entity_type: &EntityType) -> String {
    match entity_type {
        EntityType::Str(entity_type) => entity_type.to_string(),
        EntityType::Int(entity_type) => ChannelType::from(*entity_type as u8).name().to_string(),
        _ => "unknown".to_string(),
    }
}

fn format_flags_diff(old: &u64, new: &u64) -> Option<String> {
    let new_flags = ChannelFlags::from_bits(*new);
    let changed_flags = ChannelFlags::from_bits(old ^ new);

    let Some(new_flags) = new_flags else {
        return None;
    };
    let Some(changed_flags) = changed_flags else {
        return None;
    };

    let mut result = Vec::new();

    for (name, flag) in changed_flags.iter_names() {
        result.push(format!("- **{name}**: `{}`", flag.intersects(new_flags)));
    }

    if result.is_empty() {
        return None;
    }

    Some(result.join("\n"))
}

fn format_flags(flags: &u64) -> Option<String> {
    let Some(flags) = ChannelFlags::from_bits(*flags) else {
        return None;
    };

    let mut result = Vec::new();

    for (name, _flag) in flags.iter_names() {
        result.push(format!("- **{name}**: `true`"));
    }

    if result.is_empty() {
        return None;
    }

    Some(result.join("\n"))
}

fn format_access_permission(permission_overrides: &Vec<PermissionOverwrite>) -> String {
    let mut result = Vec::new();
    result.push("- **Access:**".to_string());
    for permission in permission_overrides {
        let emoji = perm_to_icon(permission.allow, permission.deny, Permissions::VIEW_CHANNEL);

        let permission_line = match permission.kind {
            PermissionOverwriteType::Role(role_id) => {
                format!("  - Role <@&{role_id}>: {emoji}")
            }
            PermissionOverwriteType::Member(user_id) => {
                format!("  - User <@{user_id}>: {emoji}")
            }
            _ => "Invalid permission".to_string(),
        };

        result.push(permission_line);
    }
    result.join("\n")
}

fn format_permission_override(allow: Permissions, deny: Permissions) -> String {
    let combined_perms = allow | deny;

    let mut result = Vec::new();

    for perm in combined_perms.iter() {
        result.push(format!("  - {perm}: {}", perm_to_icon(allow, deny, perm),));
    }

    if result.is_empty() {
        return "  - *none*".to_string();
    }

    result.join("\n")
}

fn format_permission_override_change(
    old_allow: Permissions,
    new_allow: Permissions,
    old_deny: Permissions,
    new_deny: Permissions,
) -> String {
    let perms_difference = (old_allow ^ new_allow) | (old_deny ^ new_deny);

    let mut result = Vec::new();

    for perm in perms_difference.iter() {
        result.push(format!(
            "  - {perm}: {} ➜ {}",
            perm_to_icon(old_allow, old_deny, perm),
            perm_to_icon(new_allow, new_deny, perm),
        ));
    }

    if result.is_empty() {
        return "  - *none*".to_string();
    }

    result.join("\n")
}

fn perm_to_icon(allow: Permissions, deny: Permissions, perm: Permissions) -> &'static str {
    if perm.intersects(allow) {
        return "✅";
    }
    if perm.intersects(deny) {
        return "❌";
    }
    "`╱`"
}

fn unwrap_changes(changes: &[Change]) -> String {
    let allow = find_change!(changes, Change::Allow);
    let deny = find_change!(changes, Change::Deny);

    let (allow_old, allow_new) = unwrap_change!(allow, Change::Allow);
    let (deny_old, deny_new) = unwrap_change!(deny, Change::Deny);

    let is_change =
        allow_old.is_some() && allow_new.is_some() || deny_old.is_some() && deny_new.is_some();

    if is_change {
        format_permission_override_change(
            allow_new.unwrap_or_else(Permissions::empty),
            deny_new.unwrap_or_else(Permissions::empty),
            allow_old.unwrap_or_else(Permissions::empty),
            deny_old.unwrap_or_else(Permissions::empty),
        )
    } else {
        let allow = allow_new.or(*allow_old).unwrap_or_else(Permissions::empty);
        let deny = deny_new.or(*deny_old).unwrap_or_else(Permissions::empty);

        format_permission_override(allow, deny)
    }
}
