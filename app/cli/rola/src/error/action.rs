use librorolala::protocol::ActionError;
use mingling::{
    macros::{buffer, import_type, r_eprintln, renderer},
    res::ResExitCode,
};
use rorolala_utils_cli_theme::{err_line, help_line};
use rust_i18n::t;

use crate::exit_codes::{
    EC_ERR_ACTION_ADDR, EC_ERR_ACTION_AUTH, EC_ERR_ACTION_CODEC, EC_ERR_ACTION_IO,
    EC_ERR_ACTION_JSON, EC_ERR_ACTION_MISSING_OBJECT, EC_ERR_ACTION_MISSING_VALUE,
    EC_ERR_ACTION_NO_CHANNEL, EC_ERR_ACTION_STORE, EC_ERR_ACTION_UNKNOWN,
    EC_ERR_ACTION_VALUE_TOO_LARGE,
};

// The error is foreign to this crate, so it is imported: that is what lets `?` route it
// here from a command, and what names the entry it routes to.
import_type!(ErrorAction = librorolala::protocol::ActionError);

/// Renders an action that could not run.
///
/// An [`ActionError`] is what the client side raises rather than something a command builds,
/// so it arrives through `?`: a command written with `routeify` sends it here instead of
/// saying what went wrong itself. Each way of failing is named in its own words and given an
/// exit code of its own, since which one it was is the whole of what a caller can act on.
#[renderer(buffer)]
pub fn render_error_action(err: ErrorAction, ec: &mut ResExitCode) {
    match err {
        ActionError::NoChannel => {
            r_eprintln!("{}", err_line!(t!("error.action.err_no_channel").trim()));
            r_eprintln!(
                "{}",
                help_line!(t!("error.action.err_no_channel_help").trim())
            );
            ec.exit_code = EC_ERR_ACTION_NO_CHANNEL;
        }
        ActionError::MissingValue => {
            r_eprintln!("{}", err_line!(t!("error.action.err_missing_value").trim()));
            r_eprintln!(
                "{}",
                help_line!(t!("error.action.err_missing_value_help").trim())
            );
            ec.exit_code = EC_ERR_ACTION_MISSING_VALUE;
        }
        ActionError::MissingObject => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.action.err_missing_object").trim())
            );
            r_eprintln!(
                "{}",
                help_line!(t!("error.action.err_missing_object_help").trim())
            );
            ec.exit_code = EC_ERR_ACTION_MISSING_OBJECT;
        }
        ActionError::Store(source) => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.action.err_store", reason = source).trim())
            );
            r_eprintln!("{}", help_line!(t!("error.action.err_store_help").trim()));
            ec.exit_code = EC_ERR_ACTION_STORE;
        }
        ActionError::ValueTooLarge => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.action.err_value_too_large").trim())
            );
            r_eprintln!(
                "{}",
                help_line!(t!("error.action.err_value_too_large_help").trim())
            );
            ec.exit_code = EC_ERR_ACTION_VALUE_TOO_LARGE;
        }
        ActionError::Io(source) => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.action.err_io", reason = source.to_string()).trim())
            );
            r_eprintln!("{}", help_line!(t!("error.action.err_io_help").trim()));
            ec.exit_code = EC_ERR_ACTION_IO;
        }
        ActionError::Codec(source) => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.action.err_codec", reason = source.to_string()).trim())
            );
            r_eprintln!("{}", help_line!(t!("error.action.err_codec_help").trim()));
            ec.exit_code = EC_ERR_ACTION_CODEC;
        }
        ActionError::UnknownAction(id) => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.action.err_unknown_action", id = id).trim())
            );
            r_eprintln!(
                "{}",
                help_line!(t!("error.action.err_unknown_action_help").trim())
            );
            ec.exit_code = EC_ERR_ACTION_UNKNOWN;
        }
        ActionError::Json(source) => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.action.err_json", reason = source.to_string()).trim())
            );
            r_eprintln!("{}", help_line!(t!("error.action.err_json_help").trim()));
            ec.exit_code = EC_ERR_ACTION_JSON;
        }
        ActionError::Addr(source) => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.action.err_addr", target = source.target()).trim())
            );
            r_eprintln!("{}", help_line!(t!("error.action.err_addr_help").trim()));
            ec.exit_code = EC_ERR_ACTION_ADDR;
        }
        ActionError::Auth(source) => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.action.err_auth", reason = source.to_string()).trim())
            );
            r_eprintln!("{}", help_line!(t!("error.action.err_auth_help").trim()));
            ec.exit_code = EC_ERR_ACTION_AUTH;
        }
    }
}
