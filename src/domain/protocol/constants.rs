//! Byte-compatible constants from the Java ERP protocol implementation.

pub const PROTO_CHARSET: &str = "ISO-8859-1";

pub const BOT_VERSION: &str = "7000";
pub const XOR_KEY: u8 = 69;
pub const START_MESSAGE: &str = "XONGAS";
pub const PROTO_SEPARATOR: &str = "\u{00f7}";
pub const PROTO_FINALIZER: &str = "\u{00ac}\u{00b9}\u{00b2}\u{00b3}";
pub const PROTO_ENDBLOCK: &str = "\u{00f1}\u{00b9}\u{00b2}\u{00b3}";
pub const PROTO_VERSION: &str = "\u{00a3}";
pub const PROTO_INIT: &str = "\u{00aa}";
pub const PROTO_APP_START: &str = "\u{00ba}";
pub const PROTO_GET_OS_CLIENT: &str = "\u{00e6}OS";
pub const PROTO_GETFILE: &str = "\u{00e6}FE";
pub const PROTO_PUTFILE: &str = "\u{00e6}PE";
pub const PROTO_EVENT_GETFILE: &str = "\u{00e6}F";
pub const PROTO_EVENT_PATHGETFILE: &str = "\u{00e6}FP";
pub const PROTO_EVENT_START_BROWSER: &str = "\u{00d7}\u{00df}";
pub const PROTO_EVENT_START_EDITOR: &str = "\u{00d7}\u{00c6}";
pub const PROTO_EVENT_FILE_TRANSFER: &str = "\u{00e6}4";
pub const PROTO_ADD_HIPERLINK: &str = "\u{00df}HL";
pub const PROTO_MODULES: &str = "\u{00a7}";
pub const PROTO_MENS: &str = "\u{00ba}6";
pub const PROTO_ASK: &str = "\u{00ba}7";
pub const PROTO_ERRO: &str = "\u{00ba}8";
pub const PROTO_HOME: &str = "\u{00ba}5";
pub const PROTO_INTERPRET: &str = "\u{00e6}ETI";
pub const PROTO_SERVER_EXECUTE: &str = "\u{00b5}BAS";
pub const PROTO_ALERT: &str = "\u{00ba}9";
pub const PROTO_W_MDICHILD: &str = "\u{00b0}10";
pub const PROTO_W_MDICHILD_ACTIVATE: &str = "\u{00b0}12";
pub const PROTO_W_MODAL_ACTIVATE: &str = "\u{00b0}34";
pub const PROTO_W_MDIHISTORICAL: &str = "\u{00b0}75";
pub const PROTO_W_MODALWINDOW: &str = "\u{00b0}70";
pub const PROTO_W_LABEL: &str = "\u{00b0}13";
pub const PROTO_W_BUTTON: &str = "\u{00b0}15";
pub const PROTO_W_GET: &str = "\u{00b0}11";
pub const PROTO_W_GET_KEY: &str = "\u{00b0}11K";
pub const PROTO_W_GTB: &str = "\u{00b0}14";
pub const PROTO_W_COMBO: &str = "\u{00b0}16";
pub const PROTO_W_COMBO_KEY: &str = "\u{00b0}16K";
pub const PROTO_LINES_COMBOBOX: &str = "\u{00e6}NLC";
pub const PROTO_LOOK_FOR_CREATE_DLG: &str = "\u{00be}0";
pub const PROTO_LOOK_FOR_CLICK_P1: &str = "\u{00be}1";
pub const PROTO_LOOK_FOR_CLICK_P2: &str = "\u{00be}2";
pub const PROTO_LOOK_FOR_CLICK_P4: &str = "\u{00be}4";
pub const PROTO_LOOK_FOR_CLICK_OK: &str = "\u{00be}5";
pub const PROTO_LOOK_FOR_CLICK_CANCEL: &str = "\u{00be}6";
pub const PROTO_LOOK_FOR_CLICK_LAST: &str = "\u{00be}10";
pub const PROTO_LOOK_FOR_CLICK_MORE: &str = "\u{00be}11";
pub const PROTO_LOOK_FOR_LBX_DATA: &str = "\u{00be}8";
pub const PROTO_DLG_LOOK_FOR_CREATE_DLG: &str = "\u{00d7}0";
pub const PROTO_DLG_LOOK_FOR_CLICK_P1: &str = "\u{00d7}1";
pub const PROTO_DLG_LOOK_FOR_CLICK_P2: &str = "\u{00d7}2";
pub const PROTO_DLG_LOOK_FOR_CLICK_P4: &str = "\u{00d7}4";
pub const PROTO_DLG_LOOK_FOR_CLICK_OK: &str = "\u{00d7}5";
pub const PROTO_DLG_LOOK_FOR_CLICK_CANCEL: &str = "\u{00d7}6";
pub const PROTO_DLG_LOOK_FOR_CLICK_LAST: &str = "\u{00d7}10";
pub const PROTO_DLG_LOOK_FOR_CLICK_MORE: &str = "\u{00d7}11";
pub const PROTO_DLG_LOOK_FOR_LBX_DATA: &str = "\u{00d7}8";
pub const PROTO_W_CHECKBOX: &str = "\u{00b0}17";
pub const PROTO_W_SUBBROWSE: &str = "\u{00b0}18";
pub const PROTO_W_SUBBROWSE_SEPARATOR: &str = "\u{00a6}";
pub const PROTO_W_SUBBROWSE_CONFIGURE_COLUMNS: &str = "\u{00b0}CC";
pub const PROTO_W_SUBBROWSE_NOTIFY_LINE: &str = "\u{00bc}";
pub const PROTO_W_SUBBROWSE_FOCUS_LINE: &str = "\u{00bc}\u{00fe}FL";
pub const PROTO_W_MGT: &str = "\u{00b0}19";
pub const PROTO_W_IMAGE: &str = "\u{00b0}22";
pub const PROTO_W_BOX: &str = "\u{00b0}20";
pub const PROTO_W_LABEL_ATIVO: &str = "\u{00b0}21";
pub const PROTO_W_TOOLBAR: &str = "\u{00b0}33";
pub const CLIENT_ACK_OK: &str = "\u{00ba}3";
pub const CLIENT_ACK_NOLICENCE: &str = "\u{00ba}1";
pub const CLIENT_ACK_ASK_LOGON: &str = "\u{00ba}2";
pub const PROTO_LOGON: &str = "\u{00ba}99998";
pub const PROTO_NORMAL_CLOSE: &str = "\u{00ba}99997";
pub const PROTO_EVENT_CLICK: &str = "\u{00b5}";
pub const PROTO_EVENT_DBLCLICK: &str = "\u{00b5}\u{00b0}2";
pub const PROTO_W_DLGEVENT: &str = "\u{00f8}";
pub const PROTO_EVENT_CLOSE: &str = "\u{00f0}";
pub const PROTO_EVENT_BUTTONBAR_CLICK: &str = "\u{00b5}\u{00b0}33";
pub const PROTO_EVENT_REFRESHDATA: &str = "\u{00a9}";
pub const PROTO_EVENT_AEVALWHEN: &str = "\u{00ae}";
pub const PROTO_EVENT_SETFOCUS: &str = "\u{00a1}";
pub const PROTO_W_EVENT_SETFOCUS: &str = "\u{00a1}w";
pub const PROTO_GETCLIENTDATA: &str = "\u{00a1}G";
pub const PROTO_USERS: &str = "\u{00e6}ETU";
pub const PROTO_EVENT_SETFONT: &str = "\u{00e6}SF";
pub const PROTO_EVENT_SETCOLOR: &str = "\u{00e6}C";
pub const PROTO_CTRL_VISIBLE: &str = "\u{00e6}SV";
pub const PROTO_CHANGE_BITMAP_ATTACH: &str = "\u{00e6}TBA";
pub const PROTO_EVENT_SETTEXT: &str = "\u{00e6}X";
pub const PROTO_EVENT_CLOSED: &str = "\u{00fe}";
pub const PROTO_EVENT_MODAL_CLOSED: &str = "\u{00fe}M";
pub const PROTO_EVENT_LOSTFOCUS: &str = "\u{00df}";
pub const PROTO_EVENT_HIDE: &str = "\u{00e6}6";
pub const PROTO_SET_TOOLTIP: &str = "\u{00df}TT";
pub const PROTO_CLIPBOARD: &str = "\u{00e6}CLI";
pub const MENU_TYPE_MODULE: &str = "\u{00b5}0";
pub const MENU_TYPE_CLASS: &str = "\u{00b5}1";
pub const MENU_TYPE_METHOD: &str = "\u{00b5}2";
pub const PROTO_P_INIT: &str = "^";
pub const PROTO_TOOLBAR_NEW: &str = "\u{00d7}TN";
pub const PROTO_W_MESSAGE_DATE: &str = "\u{00e4}MS";
pub const PROTO_W_HISTORIC_DATE: &str = "\u{00e4}SD";
pub const PROTO_W_DEFAULT_MESSAGE: &str = "\u{00e4}DM";
pub const PROTO_W_SERVER_VERSION: &str = "\u{00e4}SV";
pub const PROTO_PREFERENCE_CLIENT_ACTIVATE: &str = "\u{00b5}PCA";
pub const PROTO_MSGRUN_START: &str = "\u{00ba}\u{00fe}8";
pub const PROTO_MSGRUN_STOP: &str = "\u{00ba}\u{00fe}9";
pub const PACK_START: &str = "$<PACK>$";
pub const PACK_END: &str = "$<ENDPACK>$";
pub const LENGTH_END: &str = "$<ENDLEN>$";

#[cfg(test)]
mod tests {
    use super::{
        BOT_VERSION, LENGTH_END, PACK_END, PACK_START, PROTO_CHARSET, PROTO_ENDBLOCK,
        PROTO_FINALIZER, PROTO_SEPARATOR, START_MESSAGE, XOR_KEY,
    };

    #[test]
    fn preserves_core_legacy_values() {
        assert_eq!(PROTO_CHARSET, "ISO-8859-1");
        assert_eq!(PROTO_SEPARATOR, "\u{00f7}");
        assert_eq!(PROTO_FINALIZER, "\u{00ac}\u{00b9}\u{00b2}\u{00b3}");
        assert_eq!(PROTO_ENDBLOCK, "\u{00f1}\u{00b9}\u{00b2}\u{00b3}");
        assert_eq!(START_MESSAGE, "XONGAS");
        assert_eq!(BOT_VERSION, "7000");
        assert_eq!(XOR_KEY, 69);
        assert_eq!(PACK_START, "$<PACK>$");
        assert_eq!(PACK_END, "$<ENDPACK>$");
        assert_eq!(LENGTH_END, "$<ENDLEN>$");
    }
}
