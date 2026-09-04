use serenity::all::{
    AuditLogEntry, AutoModAction, Change, ChannelId, Colour, Context, CreateEmbed, CreateMessage,
    GuildId, RoleId, RuleId, User, audit_log::Action,
};

use crate::{
    find_change, format_boolean_change, format_string_change,
    messages::utils::{build_embed_author, format_user, get_name},
};

pub async fn build_automod_message(
    entry: AuditLogEntry,
    admin: Option<User>,
    guild_id: GuildId,
    ctx: &Context,
) -> Option<CreateMessage<'static>> {
    let Some(target_id) = entry.target_id else {
        log::error!("No target channel id provided");
        return None;
    };

    let Some(user_id) = entry.user_id else {
        return None;
    };

    let (action, colour) = match entry.action {
        Action::AutoMod(AutoModAction::RuleCreate) => ("created", Colour::new(0x00FF00)),
        Action::AutoMod(AutoModAction::RuleDelete) => ("deleted", Colour::new(0xFF0000)),
        Action::AutoMod(AutoModAction::RuleUpdate) => ("updated", Colour::new(0xFFAA00)),
        _ => {
            // We don't care about other actions like messages being blocked
            return None;
        }
    };

    let user_str = format_user(&admin, user_id);

    let rule_id = RuleId::new(target_id.get());

    let rule = guild_id.automod_rule(&ctx.http, rule_id).await.ok();

    let name_change = find_change!(entry.changes, Change::Name);
    let name = get_name(name_change)
        .map(str::to_string)
        .unwrap_or_else(|| {
            rule.map(|rule| rule.name.to_string())
                .unwrap_or_else(|| "*Unknown rule*".to_string())
        });

    let changes_string = entry
        .changes
        .iter()
        .filter_map(build_automod_change_line)
        .collect::<Vec<_>>()
        .join("\n");

    let embed_author = build_embed_author(&admin, user_id);
    let message = format!(
        "{user_str} **{action} automod rule** {name}**:**\n\n\
         {changes_string}"
    );
    let title = format!("AUTOMOD RULE {}", action.to_uppercase());

    let embed = CreateEmbed::new()
        .title(title)
        .author(embed_author)
        .color(colour)
        .description(message);

    Some(CreateMessage::new().embed(embed))
}

fn build_automod_change_line(change: &Change) -> Option<String> {
    Some(match change {
        Change::Name { old, new } => format_string_change!("Name", old, new),
        Change::Enabled { old, new } => format_boolean_change!("Enabled", old, new),
        /*Change::Other {
            name,
            old_value: _,
            new_value: Some(value),
        } => {
            let list = serde_json::from_value::<Vec<String>>(value.clone()).ok();
            let Some(list) = list else {
                return None;
            };

            let label = match name.as_str() {
                "$add_keyword_filter" => "Added words",
                "$remove_keyword_filter" => "Removed words",
                "$add_regex_patterns" => "Added regex",
                "$remove_regex_patterns" => "Removed regex",
                "$add_allow_list" => "Added allowed words",
                "$remove_allow_list" => "Removed allowed words",
                _ => return Some(name.to_string())
            };

            format_keyword_change(label, list)
        }*/
        Change::ExemptRoles { old, new } => {
            let mut res = Vec::new();
            if let Some(old) = old {
                res.extend(format_role_list("Removed exempt roles", old));
            }
            if let Some(new) = new {
                res.extend(format_role_list("Added exempt roles", new));
            }
            res.join("\n")
        }
        Change::ExemptChannels { old, new } => {
            let mut res = Vec::new();
            if let Some(old) = old {
                res.extend(format_channel_list("Removed exempt channels", old));
            }
            if let Some(new) = new {
                res.extend(format_channel_list("Added exempt channels", new));
            }
            res.join("\n")
        }
        // TODO
        //Change::Actions { old, new } => return None,
        _ => return None,
    })
}

/*fn format_keyword_change(label: &str, changes: Vec<String>) -> String {
    let mut res = Vec::new();
    res.push(format!("- **{label}:**"));

    for change in changes {
        res.push(format!("  - `{change}`"));
    }
    res.join("\n")
}*/

fn format_channel_list(label: &str, channels: &[ChannelId]) -> Vec<String> {
    let mut res = Vec::new();
    res.push(format!("- **{label}:**"));

    for channel in channels {
        res.push(format!("  - <#{channel}>"));
    }
    res
}

fn format_role_list(label: &str, roles: &[RoleId]) -> Vec<String> {
    let mut res = Vec::new();
    res.push(format!("- **{label}:**"));

    for role in roles {
        res.push(format!("  - <@!{role}>"));
    }
    res
}
