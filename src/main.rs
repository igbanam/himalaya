use anyhow::Result;
use clap::{self, AppSettings};
use env_logger;
use std::{convert::TryFrom, env};
use url::Url;

mod compl;
mod config;
mod domain;
mod output;
mod ui;

use config::{Account, Config};
use domain::{
    imap::{imap_arg, imap_handler, ImapService, ImapServiceInterface},
    mbox::{mbox_arg, mbox_handler, Mbox},
    msg::{flag_arg, flag_handler, msg_arg, msg_handler, tpl_arg, tpl_handler},
    smtp::SmtpService,
};
use output::OutputService;

fn create_app<'a>() -> clap::App<'a, 'a> {
    clap::App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .setting(AppSettings::GlobalVersion)
        .args(&config::config_arg::args())
        .args(&output::output_arg::args())
        .arg(mbox_arg::source_arg())
        .subcommands(compl::compl_arg::subcmds())
        .subcommands(imap_arg::subcmds())
        .subcommands(mbox_arg::subcmds())
        .subcommands(msg_arg::subcmds())
}

fn main() -> Result<()> {
    // Init env logger
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "off"),
    );

    // Check mailto match BEFORE app initialization.
    let raw_args: Vec<String> = env::args().collect();
    if raw_args.len() > 1 && raw_args[1].starts_with("mailto:") {
        let mbox = Mbox::from("INBOX");
        let config = Config::try_from(None)?;
        let account = Account::try_from((&config, None))?;
        let output = OutputService::from("plain");
        let url = Url::parse(&raw_args[1])?;
        let mut imap = ImapService::from((&account, &mbox));
        let mut smtp = SmtpService::from(&account);
        return msg_handler::mailto(&url, &account, &output, &mut imap, &mut smtp);
    }

    let app = create_app();
    let m = app.get_matches();

    // Check completion match BEFORE entities and services initialization.
    // Linked issue: https://github.com/soywod/himalaya/issues/115.
    match compl::compl_arg::matches(&m)? {
        Some(compl::compl_arg::Command::Generate(shell)) => {
            return compl::compl_handler::generate(create_app(), shell);
        }
        _ => (),
    }

    let mbox = Mbox::try_from(m.value_of("mailbox"))?;
    let config = Config::try_from(m.value_of("config"))?;
    let account = Account::try_from((&config, m.value_of("account")))?;
    let output = OutputService::try_from(m.value_of("output"))?;
    let mut imap = ImapService::from((&account, &mbox));
    let mut smtp = SmtpService::from(&account);

    // Check IMAP matches.
    match imap_arg::matches(&m)? {
        Some(imap_arg::Command::Notify(keepalive)) => {
            return imap_handler::notify(keepalive, &config, &mut imap);
        }
        Some(imap_arg::Command::Watch(keepalive)) => {
            return imap_handler::watch(keepalive, &mut imap);
        }
        _ => (),
    }

    // Check mailbox matches.
    match mbox_arg::matches(&m)? {
        Some(mbox_arg::Command::List) => {
            return mbox_handler::list(&output, &mut imap);
        }
        _ => (),
    }

    // Check message matches.
    match msg_arg::matches(&m)? {
        Some(msg_arg::Command::Attachments(seq)) => {
            return msg_handler::attachments(seq, &account, &output, &mut imap);
        }
        Some(msg_arg::Command::Copy(seq, target)) => {
            return msg_handler::copy(seq, target, &output, &mut imap);
        }
        Some(msg_arg::Command::Delete(seq)) => {
            return msg_handler::delete(seq, &output, &mut imap);
        }
        Some(msg_arg::Command::Forward(seq, atts)) => {
            return msg_handler::forward(seq, atts, &account, &output, &mut imap, &mut smtp);
        }
        Some(msg_arg::Command::List(page_size, page)) => {
            return msg_handler::list(page_size, page, &account, &output, &mut imap);
        }
        Some(msg_arg::Command::Move(seq, target)) => {
            return msg_handler::move_(seq, target, &output, &mut imap);
        }
        Some(msg_arg::Command::Read(seq, mime, raw)) => {
            return msg_handler::read(seq, mime, raw, &output, &mut imap);
        }
        Some(msg_arg::Command::Reply(seq, all, atts)) => {
            return msg_handler::reply(seq, all, atts, &account, &output, &mut imap, &mut smtp);
        }
        Some(msg_arg::Command::Save(target, msg)) => {
            return msg_handler::save(target, msg, &mut imap);
        }
        Some(msg_arg::Command::Search(query, page_size, page)) => {
            return msg_handler::search(query, page_size, page, &account, &output, &mut imap);
        }
        Some(msg_arg::Command::Send(raw_msg)) => {
            return msg_handler::send(raw_msg, &output, &mut imap, &mut smtp);
        }
        Some(msg_arg::Command::Write(atts)) => {
            return msg_handler::write(atts, &account, &output, &mut imap, &mut smtp);
        }
        Some(msg_arg::Command::Flag(m)) => match m {
            Some(flag_arg::Command::Set(seq_range, flags)) => {
                return flag_handler::set(seq_range, flags, &output, &mut imap);
            }
            Some(flag_arg::Command::Add(seq_range, flags)) => {
                return flag_handler::add(seq_range, flags, &output, &mut imap);
            }
            Some(flag_arg::Command::Remove(seq_range, flags)) => {
                return flag_handler::remove(seq_range, flags, &output, &mut imap);
            }
            _ => (),
        },
        Some(msg_arg::Command::Tpl(m)) => match m {
            Some(tpl_arg::Command::New(tpl)) => {
                return tpl_handler::new(tpl, &account, &output);
            }
            Some(tpl_arg::Command::Reply(seq, all, tpl)) => {
                return tpl_handler::reply(seq, all, tpl, &account, &output, &mut imap);
            }
            Some(tpl_arg::Command::Forward(seq, tpl)) => {
                return tpl_handler::forward(seq, tpl, &account, &output, &mut imap);
            }
            _ => (),
        },
        _ => (),
    }

    imap.logout()
}
