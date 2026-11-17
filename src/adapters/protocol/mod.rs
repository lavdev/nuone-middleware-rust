Warning: truncated output (original token count: 10489)
Total output lines: 1013

//! Outbound adapters for legacy protocol transformations.

use std::io::{Read, Write};

use flate2::{Compression, read::DeflateDecoder, write::DeflateEncoder};

use crate::{
    application::ports::json_to_proto::{JsonToProtoError, JsonToProtoPort},
    application::ports::packer::{PackerError, PackerPort},
    application::ports::proto_to_json::{ProtoToJsonError, ProtoToJsonPort},
    domain::protocol::{
        CLIENT_ACK_OK, LENGTH_END, PACK_END, PACK_START, PROTO_ADD_HIPERLINK, PROTO_ALERT,
        PROTO_ASK, PROTO_CHANGE_BITMAP_ATTACH, PROTO_CLIPBOARD, PROTO_CTRL_VISIBLE,
        PROTO_DLG_LOOK_FOR_CLICK_CANCEL, PROTO_DLG_LOOK_FOR_CLICK_LAST,
        PROTO_DLG_LOOK_FOR_CLICK_MORE, PROTO_DLG_LOOK_FOR_CLICK_OK, PROTO_DLG_LOOK_FOR_CLICK_P1,
        PROTO_DLG_LOOK_FOR_CLICK_P2, PROTO_DLG_LOOK_FOR_CLICK_P4, PROTO_DLG_LOOK_FOR_CREATE_DLG,
        PROTO_DLG_LOOK_FOR_LBX_DATA, PROTO_ENDBLOCK, PROTO_ERRO, PROTO_EVENT_AEVALWHEN,
        PROTO_EVENT_BUTTONBAR_CLICK, PROTO_EVENT_CLICK, PROTO_EVENT_CLOSE, PROTO_EVENT_CLOSED,
        PROTO_EVENT_DBLCLICK, PROTO_EVENT_FILE_TRANSFER, PROTO_EVENT_GETFILE, PROTO_EVENT_HIDE,
        PROTO_EVENT_LOSTFOCUS, PROTO_EVENT_MODAL_CLOSED, PROTO_EVENT_PATHGETFILE,
        PROTO_EVENT_REFRESHDATA, PROTO_EVENT_SETCOLOR, PROTO_EVENT_SETFOCUS, PROTO_EVENT_SETFONT,
        PROTO_EVENT_SETTEXT, PROTO_EVENT_START_BROWSER, PROTO_FINALIZER, PROTO_GETCLIENTDATA,
        PROTO_HOME, PROTO_INTERPRET, PROTO_LINES_COMBOBOX, PROTO_LOOK_FOR_CLICK_CANCEL,
        PROTO_LOOK_FOR_CLICK_LAST, PROTO_LOOK_FOR_CLICK_MORE, PROTO_LOOK_FOR_CLICK_OK,
        PROTO_LOOK_FOR_CLICK_P1, PROTO_LOOK_FOR_CLICK_P2, PROTO_LOOK_FOR_CLICK_P4,
        PROTO_LOOK_FOR_CREATE_DLG, PROTO_LOOK_FOR_LBX_DATA, PROTO_MENS, PROTO_MSGRUN_START,
        PROTO_MSGRUN_STOP, PROTO_P_INIT, PROTO_SEPARATOR, PROTO_SERVER_EXECUTE, PROTO_SET_TOOLTIP,
        PROTO_USERS, PROTO_W_BOX, PROTO_W_BUTTON, PROTO_W_CHECKBOX, PROTO_W_COMBO,
        PROTO_W_COMBO_KEY, PROTO_W_DEFAULT_MESSAGE, PROTO_W_DLGEVENT, PROTO_W_EVENT_SETFOCUS,
        PROTO_W_GET, PROTO_W_GET_KEY, PROTO_W_GTB, PROTO_W_HISTORIC_DATE, PROTO_W_IMAGE,
        PROTO_W_LABEL, PROTO_W_LABEL_ATIVO, PROTO_W_MDICHILD, PROTO_W_MDICHILD_ACTIVATE,
        PROTO_W_MDIHISTORICAL, PROTO_W_MESSAGE_DATE, PROTO_W_MGT, PROTO_W_MODAL_ACTIVATE,
        PROTO_W_MODALWINDOW, PROTO_W_SERVER_VERSION, PROTO_W_SUBBROWSE,
        PROTO_W_SUBBROWSE_CONFIGURE_COLUMNS, PROTO_W_SUBBROWSE_FOCUS_LINE,
        PROTO_W_SUBBROWSE_NOTIFY_LINE, PROTO_W_SUBBROWSE_SEPARATOR, PROTO_W_TOOLBAR,
        encode_proto_text,
    },
};

const MAX_UNIT_LENGTH: usize = 200;
const MAX_PACK_SIZE: usize = 10 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default)]
pub struct DeflatePackerAdapter;

#[derive(Clone, Copy, Debug, Default)]
pub struct ProtoToJsonAdapter;

#[derive(Clone, Copy, Debug, Default)]
pub struct JsonToProtoAdapter;

impl JsonToProtoPort for JsonToProtoAdapter {
    fn translate(&self, command_json: &str) -> Result<String, JsonToProtoError> {
        let root: serde_json::Value =
            serde_json::from_str(command_json).map_err(|_| JsonToProtoError::InvalidJson)?;
        let command = root.get("command").unwrap_or(&root);

        if let Some(menu_item) = command.get("menuitem") {
            return Ok(join_proto(&[PROTO_EVENT_CLICK, &value_text(menu_item)]));
        }
        if command.get("click").is_some() {
            return translate_click(command);
        }
        if command.get("erp_initialization").is_some() {
            return Ok(PROTO_P_INIT.to_owned());
        }
        if command.get("getfile").is_some() || command.get("GETFILE").is_some() {
            let filename = if command.get("getfile").is_some() {
                "getfile"
            } else {
                "GETFILE"
            };
            let filename = required_text(command, filename)?.replace(' ', "_");
            return Ok(join_proto(&["GETFILE", &filename]));
        }
        if let Some(window) = command.get("windowClose") {
            return Ok(join_proto(&[
                PROTO_W_DLGEVENT,
                PROTO_EVENT_CLOSE,
                &required_text_value(window, "windowClose")?,
                "?",
            ]));
        }
        if let Some(update) = command.get("UpdateComponent") {
            let value = required_text(update, "value")?.replace('\n', "\r\n");
            return Ok(join_proto(&[
                PROTO_W_DLGEVENT,
                PROTO_EVENT_LOSTFOCUS,
                &required_text(update, "parent")?,
                &required_text(update, "id")?,
                &value,
            ]));
        }
        if let Some(selected) = command.get("SelectedRow") {
            return translate_selected_row(selected);
        }
        if let Some(developer) = command.get("developer") {
            let action = required_text(developer, "action")?;
            return if action.contains("interpreter") {
                Ok(PROTO_INTERPRET.to_owned())
            } else {
                Err(JsonToProtoError::UnsupportedCommand)
            };
        }
        if let Some(misc) = command.get("Misc") {
            return match value_text(misc).as_str() {
                "shortcut" => Ok(PROTO_HOME.to_owned()),
                "users" => Ok(PROTO_USERS.to_owned()),
                _ => Err(JsonToProtoError::UnsupportedCommand),
            };
        }
        if let Some(link) = command.get("Hiperlink") {
            return Ok(join_proto(&[
                PROTO_SERVER_EXECUTE,
                &required_text(link, "action")?,
            ]));
        }
        if let Some(transaction) = command.get("transaction") {
            return Ok(join_proto(&[
                PROTO_SERVER_EXECUTE,
                &required_text(transaction, "run")?,
            ]));
        }
        if let Some(search) = command.get("SearchMaxLine") {
            return required_text(search, "lines");
        }
        if let Some(click) = command.get("SearchButtonClick") {
            return translate_search_button(click);
        }
        if let Some(close) = command.get("SearchClose") {
            return Ok(if bool_field(close, "window") {
                PROTO_DLG_LOOK_FOR_CLICK_CANCEL
            } else {
                PROTO_LOOK_FOR_CLICK_CANCEL
            }
            .to_owned());
        }
        if let Some(client_data) = command.get("ClientData") {
            return translate_client_data(client_data);
        }
        if let Some(message_box) = command
            .get("MessageBox")
            .or_else(|| command.get("messageBox"))
        {
            return required_text(message_box, "action");
        }
        if command.get("choice").is_some() {
            return required_text(command, "choice");
        }
        if command.get("id").is_some() {
            return required_text(command, "id");
        }
        Err(JsonToProtoError::UnsupportedCommand)
    }
}

fn translate_click(command: &serde_json::Value) -> Result<String, JsonToProtoError> {
    let click_type = required_text(command, "click")?;
    match click_type.as_str() {
        "NButton" => Ok(join_proto(&[
            PROTO_W_DLGEVENT,
            PROTO_EVENT_CLICK,
            &required_text(command, "parent")?,
            &required_text(command, "id")?,
        ])),
        "NCheckBox" => Ok(join_proto(&[
            PROTO_W_DLGEVENT,
            PROTO_EVENT_CLICK,
            &required_text(command, "parent")?,
            &required_text(command, "id")?,
            &required_text(command, "value")?,
        ])),
        "buttonBar" => {
            let parent = required_text(command, "parent")?;
            let item = required_text(command, "toolbarItem")?;
            if item == "18" {
                Ok(join_proto(&[
                    PROTO_W_DLGEVENT,
                    PROTO_CLIPBOARD,
                    &parent,
                    "1",
                ]))
            } else {
                Ok(join_proto(&[PROTO_EVENT_BUTTONBAR_CLICK, &parent, &item]))
            }
        }
        _ => Err(JsonToProtoError::UnsupportedCommand),
    }
}

fn translate_selected_row(row: &serde_json::Value) -> Result<String, JsonToProtoError> {
    let source = required_text(row, "source")?;
    if source.eq_ignore_ascii_case("search") {
        let header = if bool_field(row, "window") {
            PROTO_DLG_LOOK_FOR_CLICK_OK
        } else {
            PROTO_LOOK_FOR_CLICK_OK
        };
        return Ok(join_proto(&[header, &required_text(row, "Row")?]));
    }
    if source.eq_ignore_ascii_case("grid") || source.eq_ignore_ascii_case("NGRID") {
        let event = if bool_field(row, "doubleClick") {
            PROTO_EVENT_DBLCLICK
        } else {
            PROTO_W_SUBBROWSE_NOTIFY_LINE
        };
        return Ok(join_proto(&[
            PROTO_W_DLGEVENT,
            event,
            &required_text(row, "parent")?,
            &required_text(row, "id")?,
            &required_text(row, "Row")?,
        ]));
    }
    Err(JsonToProtoError::UnsupportedCommand)
}

fn translate_search_button(click: &serde_json::Value) -> Result<String, JsonToProtoError> {
    let button = required_text(click, "button")?.to_uppercase();
    let dialog = bool_field(click, "window");
    let header = match (button.as_str(), dialog) {
        ("1" | "FIRST", false) => PROTO_LOOK_FOR_CLICK_P1,
        ("1" | "FIRST", true) => PROTO_DLG_LOOK_FOR_CLICK_P1,
        ("2", false) => PROTO_LOOK_FOR_CLICK_P2,
        ("2", true) => PROTO_DLG_LOOK_FOR_CLICK_P2,
        ("4", false) => PROTO_LOOK_FOR_CLICK_P4,
        ("4", true) => PROTO_DLG_LOOK_FOR_CLICK_P4,
        ("LAST", false) => PROTO_LOOK_FOR_CLICK_LAST,
        ("LAST", true) => PROTO_DLG_LOOK_FOR_CLICK_LAST,
        ("MORE", false) => PROTO_LOOK_FOR_CLICK_MORE,
        ("MORE", true) => PROTO_DLG_LOOK_FOR_CLICK_MORE,
        _ => return Err(JsonToProtoError::UnsupportedCommand),
    };
    let order = click
        .get("order")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0)
        .to_string();
    Ok(join_proto(&[
        header,
        &order,
        &required_text(click, "value")?,
        &required_text(click, "maxLine")?,
    ]))
}

fn translate_client_data(items: &serde_json::Value) -> Result<String, JsonToProtoError> {
    let Some(items) = items.as_array() else {
        return Err(JsonToProtoError::ClientDataMustBeArray);
    };
    let mut proto = String::new();
    for item in items {
        proto.push_str(PROTO_SEPARATOR);
        proto.push_str(&required_text(item, "id")?);
        proto.push_str(PROTO_SEPARATOR);
        proto.push_str(&required_text(item, "value")?);
    }
    Ok(proto)
}

fn required_text(node: &serde_json::Value, field: &str) -> Result<String, JsonToProtoError> {
    let value = node
        .get(field)
        .ok_or_else(|| JsonToProtoError::MissingField(field.to_owned()))?;
    required_text_value(value, field)
}

fn required_text_value(value: &serde_json::Value, field: &str) -> Result<String, JsonToProtoError> {
    if value.is_null() {
        return Err(JsonToProtoError::MissingField(field.to_owned()));
    }
    Ok(value_text(value))
}

fn value_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::Bool(value) => value.to_string(),
        _ => String::new(),
    }
}

fn bool_field(node: &serde_json::Value, field: &str) -> bool {
    node.get(field)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn join_proto(values: &[&str]) -> String {
    values.join(PROTO_SEPARATOR)
}

impl ProtoToJsonPort for ProtoToJsonAdapter {
    fn translate(&self, response: &str) -> Result<String, ProtoToJsonError> {
        let mut components = Vec::new();
        let mut events: Vec<serde_json::Value> = Vec::new();
        let normalized = response
            .replace(PROTO_FINALIZER, "")
            .replace("\u{fffd}\u{fffd}\u{fffd}", "");

        for block in normalized
            .split(PROTO_ENDBLOCK)
            .filter(|block| !block.trim().is_empty())
        {
            let fields: Vec<&str> = block.split(PROTO_SEPARATOR).collect();
            if fields.len() < 2 {
                continue;
            }
            let unit = ProtoUnit {
                message_id: fields[0],
                header: fields[1],
                values: &fields[2..],
            };
            match unit.header {
                CLIENT_ACK_OK | PROTO_W_SERVER_VERSION => {}
                PROTO_MENS | PROTO_ASK | PROTO_ERRO => components.push(json_component(
                    "MessageBox",
                    serde_json::json!({
                        "text": unit.get(0),
                        "title": unit.get(1).replace(PROTO_FINALIZER, ""),
                        "error": unit.header == PROTO_ERRO,
                        "question": unit.header == PROTO_ASK,
                        "messageID": unit.message_id,
                    }),
                )),
                PROTO_ALERT => {
                    let mut options = format!("{},", unit.get(2));
                    for value in unit.values.iter().skip(3).filter(|value| !value.is_empty()) {
                        options.push_str(value);
                        options.push(',');
                    }
                    components.push(json_component(
                        "Alert",
                        serde_json::json!({
                            "id": unit.message_id,
                            "text": unit.get(0),
                            "title": unit.get(1),
                            "options": options,
                        }),
                    ));
                }
                PROTO_W_MDICHILD | PROTO_W_MDIHISTORICAL => components.push(json_component(
                    "Window",
                    serde_json::json!({
                        "id": unit.message_id,
                        "title": unit.get(0),
                        "top": unit.get(2),
                        "left": unit.get(1),
                        "width": unit.get(3),
                        "height": unit.get(4),
                        "modal": "false",
                        "historic": unit.header == PROTO_W_MDIHISTORICAL,
                    }),
                )),
                PROTO_W_MODALWINDOW => components.push(json_component(
                    "Window",
                    serde_json::json!({
                        "id": unit.message_id,
                        "title": unit.get(0),
                        "width": unit.get(1),
                        "height": unit.get(2),
                        "modal": true,
                    }),
                )),
                PROTO_W_LABEL | PROTO_W_LABEL_ATIVO => {
                    let name = if unit.header == PROTO_W_LABEL_ATIVO {
                        "LabelActive"
                    } else {
                        "Label"
                    };
                    components.push(json_component(
                        name,
                        serde_json::json!({
                            "id": unit.message_id,
                            "parent": unit.get(0),
                            "text": unit.get(1),
                            "left": unit.get(2),
                            "top": unit.get(3),
                            "width": unit.get(4),
                            "height": unit.get(5),
                        }),
                    ));
                }
                PROTO_W_BUTTON => {
                    let text = normalize_button_text(unit.get(1));
                    components.push(json_component(
                        "Button",
                        serde_json::json!({
                            "id": unit.message_id,
                            "parent": unit.get(0),
                            "text": text,
                            "left": unit.get(2),
                            "top": unit.get(3),
                            "width": unit.get(4),
                            "height": unit.get(5),
                        }),
                    ));
                }
                PROTO_W_GET | PROTO_W_GET_KEY => {
                    components.push(json_component("TextField", text_field_data(&unit)))
                }
                PROTO_W_GTB => components.push(json_component(
                    "GenericField",
                    serde_json::json!({
                        "id": unit.message_id, "parent": unit.get(0), "text": unit.get(1),
                        "left": unit.get(2), "top": unit.get(3), "width": unit.get(4),
                        "height": unit.get(5),
                    }),
                )),
                PROTO_W_CHECKBOX => components.push(json_component(
                    "Checkbox",
                    serde_json::json!({
                        "id": unit.message_id, "parent": unit.get(0), "text": unit.get(1),
                        "left": unit.get(3), "top": unit.get(4), "width": unit.get(5),
                        "height": unit.get(6),
                    }),
                )),
                PROTO_W_COMBO | PROTO_W_COMBO_KEY => {
                    let data = unit
                        .values
                        .get(7..)
                        .unwrap_or_default()
                        .chunks_exact(2)
                        .map(|pair| {
                            serde_json::json!({
                                "name": pair[0].trim(), "value": pair[1].trim()
                            })
                        })
                        .collect::<Vec<_>>();
                    components.push(json_component(
                        "ComboBox",
                        serde_json::json!({
                            "id": unit.message_id, "parent": unit.get(0),
                            "left": unit.get(3), "top": unit.get(4),
                            "w…1489 tokens truncated…        }
                PROTO_EVENT_CLOSED | PROTO_EVENT_MODAL_CLOSED => {
                    events.push(serde_json::json!({ "windowClose": unit.get(0) }));
                }
                PROTO_W_SUBBROWSE_NOTIFY_LINE => {
                    let mut update = serde_json::json!({
                        "parent": unit.get(0),
                        "id": unit.get(1),
                        "row": unit.get(2),
                    });
                    if unit.values.len() > 3 {
                        let value = unit
                            .get(3)
                            .replace(&format!("'{PROTO_W_SUBBROWSE_SEPARATOR}"), "'^");
                        update["value"] = serde_json::json!(value);
                    }
                    events.push(json_event("GridUpdateRow", update));
                }
                PROTO_EVENT_AEVALWHEN | PROTO_CTRL_VISIBLE => {
                    let key = if unit.header == PROTO_EVENT_AEVALWHEN {
                        "enable"
                    } else {
                        "visible"
                    };
                    let mut controls = Vec::new();
                    for pair in unit.values.get(1..).unwrap_or_default().chunks_exact(2) {
                        let mut control = serde_json::json!({
                            "parent": unit.get(0),
                            "id": pair[0],
                        });
                        control[key] = serde_json::json!(pair[1] == "1");
                        controls.push(control);
                    }
                    let mut event = serde_json::Map::new();
                    event.insert(key.to_owned(), serde_json::json!(controls));
                    events.push(serde_json::Value::Object(event));
                }
                PROTO_EVENT_HIDE => events.push(json_event(
                    "Hide",
                    serde_json::json!({
                        "parent": unit.get(0),
                        "id": unit.get(1),
                        "value": false,
                    }),
                )),
                PROTO_EVENT_SETTEXT => {
                    let mut data = serde_json::json!({
                        "parent": unit.get(0),
                        "id": unit.get(1),
                    });
                    if unit.values.len() > 2 {
                        data["value"] = serde_json::json!(unit.get(2));
                    }
                    events.push(json_event("SetText", data));
                }
                PROTO_EVENT_SETFONT => {
                    let mut properties = Vec::new();
                    for pair in unit.values.get(1..).unwrap_or_default().chunks_exact(2) {
                        let parts = pair[1].split(';').collect::<Vec<_>>();
                        if parts.len() < 5 {
                            continue;
                        }
                        let Ok(size) = parts[4].replace(',', ".").parse::<f64>() else {
                            return Err(ProtoToJsonError::Translation);
                        };
                        properties.push(serde_json::json!({
                            "id": pair[0], "font": parts[0], "bold": parts[1] == "1",
                            "italic": parts[2] == "1", "hagingBaseline": parts[3] == "1",
                            "size": size,
                        }));
                    }
                    events.push(json_event(
                        "font",
                        serde_json::json!({
                            "parent": unit.get(0), "properties": properties,
                        }),
                    ));
                }
                PROTO_EVENT_SETCOLOR => {
                    let properties = unit
                        .values
                        .get(1..)
                        .unwrap_or_default()
                        .chunks_exact(3)
                        .map(|item| {
                            serde_json::json!({
                                "id": item[0], "background": item[1], "foreground": item[2],
                            })
                        })
                        .collect::<Vec<_>>();
                    events.push(json_event(
                        "color",
                        serde_json::json!({
                            "parent": unit.get(0), "properties": properties,
                        }),
                    ));
                }
                PROTO_CHANGE_BITMAP_ATTACH => events.push(json_event(
                    "ToolbarIcon",
                    serde_json::json!({
                        "parent": unit.get(0),
                        "icon": if unit.get(1) == "1" { "file-circle-check" } else { "file" },
                    }),
                )),
                PROTO_EVENT_SETFOCUS => events.push(json_event(
                    "Focus",
                    serde_json::json!({ "parent": unit.get(0), "id": unit.get(1) }),
                )),
                PROTO_W_EVENT_SETFOCUS => events.push(json_event(
                    "Focus",
                    serde_json::json!({ "parent": unit.get(0) }),
                )),
                PROTO_GETCLIENTDATA => {
                    events.push(serde_json::json!({ "ClientData": unit.get(0) }));
                }
                PROTO_W_MESSAGE_DATE | PROTO_W_HISTORIC_DATE | PROTO_W_DEFAULT_MESSAGE => {
                    let name = match unit.header {
                        PROTO_W_MESSAGE_DATE => "StatusInfo",
                        PROTO_W_HISTORIC_DATE => "StatusInfoHistoricDate",
                        _ => "StatusInfoDefaultMessage",
                    };
                    events.push(json_event(
                        name,
                        serde_json::json!({ "value": unit.get(0) }),
                    ));
                }
                PROTO_EVENT_GETFILE | PROTO_EVENT_PATHGETFILE => {
                    let mut data = serde_json::json!({
                        "id": unit.message_id,
                        "mimetype": unit.get(0),
                    });
                    if !unit.get(1).is_empty() {
                        data["text"] = serde_json::json!(unit.get(1));
                    }
                    events.push(json_event("getfile", data));
                }
                PROTO_EVENT_START_BROWSER => events.push(json_event(
                    "StartBrowser",
                    serde_json::json!({ "id": unit.message_id, "url": unit.get(0) }),
                )),
                PROTO_EVENT_FILE_TRANSFER => events.push(json_event(
                    "FileDownload",
                    serde_json::json!({
                        "id": unit.message_id,
                        "url": unit.get(0),
                        "filename": transfer_filename(unit.get(0)),
                    }),
                )),
                PROTO_ADD_HIPERLINK => events.push(json_event(
                    "Hiperlink",
                    serde_json::json!({
                        "id": unit.message_id,
                        "label": unit.get(0),
                        "action": unit.get(1),
                        "image": unit.get(2),
                        "top": unit.get(3),
                        "left": unit.get(4),
                    }),
                )),
                PROTO_SET_TOOLTIP => {
                    let components = unit
                        .values
                        .get(1..)
                        .unwrap_or_default()
                        .chunks(2)
                        .filter_map(|pair| {
                            let id = pair.first().copied().unwrap_or_default();
                            let value = pair.get(1).copied().unwrap_or_default();
                            (!id.is_empty())
                                .then(|| serde_json::json!({ "id": id, "value": value }))
                        })
                        .collect::<Vec<_>>();
                    events.push(json_event(
                        "Tooltip",
                        serde_json::json!({ "parent": unit.get(0), "component": components }),
                    ));
                }
                PROTO_W_MDICHILD_ACTIVATE | PROTO_W_MODAL_ACTIVATE => {
                    events.push(serde_json::json!({ "windowActivate": unit.message_id }));
                }
                PROTO_W_SUBBROWSE_FOCUS_LINE => events.push(json_event(
                    "GridFocusRow",
                    serde_json::json!({
                        "parent": unit.get(0),
                        "id": unit.get(1),
                        "row": unit.get(2),
                    }),
                )),
                PROTO_LINES_COMBOBOX => events.push(json_event(
                    "SearchMaxLine",
                    serde_json::json!({ "lines": "15" }),
                )),
                PROTO_LOOK_FOR_CREATE_DLG | PROTO_DLG_LOOK_FOR_CREATE_DLG => {
                    let order = unit.get(1).parse::<i64>().unwrap_or(0);
                    let values = unit
                        .values
                        .iter()
                        .skip(4)
                        .map(|value| value.replace('"', ""))
                        .collect::<Vec<_>>();
                    events.push(json_event(
                        "Search",
                        serde_json::json!({
                            "order": order,
                            "maxLine": unit.get(2),
                            "title": unit.get(3),
                            "window": unit.header == PROTO_DLG_LOOK_FOR_CREATE_DLG,
                            "ComboBoxValue": values,
                        }),
                    ));
                }
                PROTO_LOOK_FOR_LBX_DATA | PROTO_DLG_LOOK_FOR_LBX_DATA => {
                    events.push(serde_json::json!({
                        "SearchGridData": unit.values.iter().map(|value| *value).collect::<Vec<_>>()
                    }));
                }
                _ => {}
            }
        }

        serde_json::to_string(&serde_json::json!({
            "content": { "Component": components, "Event": events }
        }))
        .map_err(|_| ProtoToJsonError::Translation)
    }
}

struct ProtoUnit<'a> {
    message_id: &'a str,
    header: &'a str,
    values: &'a [&'a str],
}

impl ProtoUnit<'_> {
    fn get(&self, index: usize) -> &str {
        self.values.get(index).copied().unwrap_or_default()
    }
}

fn json_component(name: &str, data: serde_json::Value) -> serde_json::Value {
    let mut component = serde_json::Map::new();
    component.insert(name.to_owned(), data);
    serde_json::Value::Object(component)
}

fn json_event(name: &str, data: serde_json::Value) -> serde_json::Value {
    let mut event = serde_json::Map::new();
    event.insert(name.to_owned(), data);
    serde_json::Value::Object(event)
}

fn text_field_data(unit: &ProtoUnit<'_>) -> serde_json::Value {
    let mut data = serde_json::json!({
        "id": unit.message_id,
        "parent": unit.get(0),
        "text": unit.get(1),
        "left": unit.get(2),
        "top": unit.get(3),
        "width": unit.get(4),
        "height": unit.get(5),
        "key": unit.header == PROTO_W_GET_KEY,
        "fieldType": unit.get(7),
        "length": unit.get(8),
        "fieldDec": unit.get(9),
    });
    if unit.values.len() > 10 {
        data["mask"] = serde_json::json!(unit.get(10));
    }
    if unit.values.len() > 11 {
        data["isCombo"] = serde_json::json!(unit.get(11));
    }
    data
}

fn toolbar_buttons(values: &[&str]) -> Vec<serde_json::Value> {
    let mut buttons = Vec::new();
    let mut historic = Vec::new();
    for (offset, value) in values.iter().enumerate().skip(1) {
        if *value != "1" {
            continue;
        }
        let button_index = offset + 1;
        let (icon, text) = match button_index {
            2 => ("close", "Sair"),
            3 => ("save", "Salvar"),
            4 => ("add", "Incluir"),
            5 => ("angles-left", "Primeiros"),
            6 => ("angle-left", "Anterior"),
            7 => ("angle-right", "Pr�ximo"),
            8 => ("angles-right", "�ltimos"),
            9 => ("eraser", "Apagar"),
            13 => ("magnifying-glass", "Pesquisar"),
            14..=16 => ("book-bookmark", "Salvar Vers�o Atual"),
            17 => ("bookmark", "Vers�o Anterior"),
            18 => ("angles-right", "Pr�xima Vers�o"),
            19 => ("file", "Anexos"),
            _ => continue,
        };
        let button = serde_json::json!({ "icon": icon, "text": text, "id": button_index - 1 });
        if (16..=18).contains(&button_index) {
            historic.push(button);
        } else {
            buttons.push(button);
        }
    }
    buttons.extend(historic);
    buttons
}

fn transfer_filename(url: &str) -> &str {
    url.split('?')
        .next()
        .unwrap_or_default()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
}

fn normalize_button_text(text: &str) -> String {
    let replacements = [
        ("[+]", "[plus]"),
        ("[V]", "[check]"),
        ("[v]", "[check]"),
        ("[...]", "[searchengin]"),
    ];
    let Some(end) = text.find(']') else {
        return text.to_owned();
    };
    if !text.starts_with('[') {
        return text.to_owned();
    }
    let token = &text[..=end];
    let Some((_, replacement)) = replacements.iter().find(|(legacy, _)| *legacy == token) else {
        return text.to_owned();
    };
    let label = &text[end + 1..];
    if !label.is_empty() && !label.chars().next().is_some_and(char::is_whitespace) {
        format!("{replacement} {label}")
    } else {
        format!("{replacement}{label}")
    }
}

impl PackerPort for DeflatePackerAdapter {
    fn pack(&self, message: &[u8]) -> Result<Vec<u8>, PackerError> {
        if message.len() <= MAX_UNIT_LENGTH || message.len() > MAX_PACK_SIZE {
            return Ok(message.to_vec());
        }

        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
        encoder
            .write_all(message)
            .map_err(|_| PackerError::Compression)?;
        let compressed = encoder.finish().map_err(|_| PackerError::Compression)?;

        let mut packed =
            Vec::with_capacity(PACK_START.len() + compressed.len() + PACK_END.len() + 24);
        packed.extend_from_slice(&encode_proto_text(PACK_START));
        packed.extend_from_slice(&compressed);
        packed.extend_from_slice(&encode_proto_text(PACK_END));
        packed.extend_from_slice(message.len().to_string().as_bytes());
        packed.extend_from_slice(&encode_proto_text(LENGTH_END));
        Ok(packed)
    }

    fn unpack(&self, packed_message: &[u8]) -> Result<Vec<u8>, PackerError> {
        if !self.is_packed(packed_message) {
            return Ok(packed_message.to_vec());
        }
        let start = encode_proto_text(PACK_START);
        let end = encode_proto_text(PACK_END);
        let payload_start = start.len();
        let payload_end =
            find_subslice(packed_message, &end, payload_start).ok_or(PackerError::Malformed)?;

        let mut decoder = DeflateDecoder::new(&packed_message[payload_start..payload_end]);
        let mut unpacked = Vec::new();
        decoder
            .read_to_end(&mut unpacked)
            .map_err(|_| PackerError::Malformed)?;
        Ok(unpacked)
    }

    fn is_packed(&self, message: &[u8]) -> bool {
        let start = encode_proto_text(PACK_START);
        let end = encode_proto_text(PACK_END);
        message.starts_with(&start) && find_subslice(message, &end, start.len()).is_some()
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from > haystack.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|position| from + position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};

    #[test]
    fn leaves_short_messages_as_copies() {
        let packer = DeflatePackerAdapter;
        let message = b"short-proto-message";
        let packed = packer.pack(message).expect("small message");
        assert_eq!(packed, message);
        assert!(!packer.is_packed(&packed));
    }

    #[test]
    fn preserves_legacy_envelope_and_round_trips() {
        let packer = DeflatePackerAdapter;
        let message = b"PROTO_FIELD;".repeat(30);
        let packed = packer.pack(&message).expect("pack succeeds");
        assert!(packer.is_packed(&packed));
        let envelope = String::from_utf8_lossy(&packed);
        assert!(envelope.starts_with(PACK_START));
        assert!(envelope.contains(PACK_END));
        assert!(envelope.ends_with(&format!("{}{}", message.len(), LENGTH_END)));
        assert_eq!(packer.unpack(&packed).expect("unpack succeeds"), message);
    }

    #[test]
    fn unpacks_java_raw_deflate_fixture() {
        let message = b"PROTO_FIELD;".repeat(30);
        let java_envelope = [
            encode_proto_text(PACK_START),
            STANDARD
                .decode("CwjyD/GPd/N09XGxDhhl04wNAA==")
                .expect("fixture base64"),
            encode_proto_text(PACK_END),
            message.len().to_string().into_bytes(),
            encode_proto_text(LENGTH_END),
        ]
        .concat();
        assert_eq!(
            DeflatePackerAdapter
                .unpack(&java_envelope)
                .expect("Java deflate fixture unpacks"),
            message
        );
    }

    #[test]
    fn rejects_malformed_compressed_payload() {
        let packed = [
            encode_proto_text(PACK_START),
            b"bad".to_vec(),
            encode_proto_text(PACK_END),
        ]
        .concat();
        assert!(matches!(
            DeflatePackerAdapter.unpack(&packed),
            Err(PackerError::Malformed)
        ));
    }
}

