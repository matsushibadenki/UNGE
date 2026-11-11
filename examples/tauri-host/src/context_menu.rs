//! Native context menus for the GPU canvas. Actions use the existing controls transport.
use serde_json::{Value, json};
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use tauri::{
    Emitter, Manager,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
};
use unge_core::Locale;
use unge_tauri::{CanvasContext, ContextTarget};

#[derive(Default)]
pub struct Actions {
    generation: AtomicU64,
    items: Mutex<Vec<(String, Value)>>,
}
pub fn on_menu_event(app: &tauri::AppHandle, event: MenuEvent) {
    let Some(actions) = app.try_state::<Actions>() else {
        return;
    };
    let action = actions.items.lock().ok().and_then(|mut items| {
        let index = items.iter().position(|(id, _)| id == event.id().as_ref())?;
        let action = items.swap_remove(index).1;
        items.clear();
        Some(action)
    });
    if let (Some(action), Some(view)) = (action, app.get_webview("controls")) {
        let _ = view.emit("unge://canvas-action", action);
    }
}

pub fn show(
    window: &tauri::Window,
    context: CanvasContext,
    position: [f32; 2],
) -> tauri::Result<()> {
    let app = window.app_handle();
    let state = app.state::<Actions>();
    let generation = state.generation.fetch_add(1, Ordering::Relaxed);
    let menu = Menu::new(app)?;
    let mut actions = Vec::new();
    let tr = |en, ja, zh| match context.locale {
        Locale::En => en,
        Locale::Ja => ja,
        Locale::ZhCn => zh,
    };
    let rev = context.revision;
    let mut item = |name, enabled, action: Value| -> tauri::Result<MenuItem<_>> {
        let id = format!("canvas-{generation}-{}", actions.len());
        let mut action = action;
        action["position"] = json!(position);
        actions.push((id.clone(), action));
        MenuItem::with_id(app, id, name, enabled, None::<&str>)
    };
    macro_rules! add {
        ($en:expr,$ja:expr,$zh:expr,$enabled:expr,$action:expr) => {
            menu.append(&item(tr($en, $ja, $zh), $enabled, $action)?)?
        };
    }
    let heading = |text: String| -> tauri::Result<()> {
        menu.append(&MenuItem::new(app, text, false, None::<&str>)?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)
    };
    let request = |request| json!({"kind":"request","request":request});
    let apply =
        |command| request(json!({"kind":"apply","expected_revision":rev,"command":command}));
    let group = |action| request(json!({"kind":"group","expected_revision":rev,"action":action}));
    let control = |id| json!({"kind":"control","id":id,"revision":rev});
    let select = |ids: Vec<unge_core::Id>| request(json!({"kind":"select","ids":ids}));
    match context.target {
        ContextTarget::Node { id, rect, selected } => {
            heading(tr("Node", "ノード", "节点").into())?;
            add!(
                "Edit properties…",
                "プロパティを編集…",
                "编辑属性…",
                true,
                json!({"kind":"properties","id":id,"revision":rev})
            );
            add!(
                "Select node",
                "ノードを選択",
                "选中节点",
                true,
                select(vec![id])
            );
            add!(
                "Set coordinates…",
                "座標を設定…",
                "设置坐标…",
                true,
                json!({"kind":"move","id":id,"rect":rect,"revision":rev})
            );
            add!(
                "Create group…",
                "グループを作成…",
                "创建组…",
                true,
                json!({"kind":"group_name","select":if selected {None} else {Some(id)},"revision":rev})
            );
            add!(
                "Delete node",
                "ノードを削除",
                "删除节点",
                true,
                apply(json!({"kind":"remove_node","id":id}))
            );
        }
        ContextTarget::Group(g) => {
            heading(g.label.chars().take(80).collect())?;
            let name = if g.collapsed {
                tr("Expand group", "グループを展開", "展开组")
            } else {
                tr("Collapse group", "グループを折りたたむ", "折叠组")
            };
            menu.append(&item(
                name,
                g.members > 0,
                group(json!({"kind":"set_collapsed","id":g.id,"collapsed":!g.collapsed})),
            )?)?;
            add!(
                "Rename group…",
                "名前を変更…",
                "重命名组…",
                true,
                json!({"kind":"group_name","id":g.id,"label":g.label,"revision":rev})
            );
            add!(
                "Select members",
                "所属ノードを選択",
                "选中成员",
                true,
                group(json!({"kind":"select_members","id":g.id}))
            );
            add!(
                "Add selected nodes",
                "選択ノードを追加",
                "添加所选节点",
                context.selection_count > 0,
                group(json!({"kind":"add_selection","id":g.id}))
            );
            add!(
                "Remove selected nodes",
                "選択ノードを除外",
                "移除所选节点",
                context.selection_count > 0,
                group(json!({"kind":"remove_selection","id":g.id}))
            );
            add!(
                "Delete group",
                "グループを削除",
                "删除组",
                true,
                group(json!({"kind":"delete","id":g.id}))
            );
        }
        ContextTarget::Port {
            endpoint,
            output,
            label,
            edges,
        } => {
            let direction = if output {
                tr("Output", "出力", "输出")
            } else {
                tr("Input", "入力", "输入")
            };
            heading(format!("{direction} · {label}"))?;
            add!(
                "Start / finish connection",
                "接続を開始／確定",
                "开始或完成连接",
                true,
                json!({"kind":"port","endpoint":endpoint,"output":output,"revision":rev})
            );
            add!(
                "Edit node properties…",
                "ノードのプロパティを編集…",
                "编辑节点属性…",
                true,
                json!({"kind":"properties","id":endpoint.node,"revision":rev})
            );
            let enabled = !edges.is_empty() && edges.len() <= unge_tauri::MAX_CONTEXT_PORT_EDGES;
            let commands: Vec<_> = if enabled {
                edges
                    .into_iter()
                    .map(|id| json!({"kind":"disconnect","id":id}))
                    .collect()
            } else {
                Vec::new()
            };
            add!(
                "Disconnect visible connections",
                "表示中の接続を解除",
                "断开可见连接",
                enabled,
                apply(json!({"kind":"batch","commands":commands}))
            );
        }
        ContextTarget::Edge {
            id,
            from,
            to,
            labels,
        } => {
            heading(format!("{} → {}", labels[0], labels[1]))?;
            add!(
                "Select source node",
                "接続元を選択",
                "选中源节点",
                true,
                select(vec![from.node])
            );
            add!(
                "Select destination node",
                "接続先を選択",
                "选中目标节点",
                true,
                select(vec![to.node])
            );
            add!(
                "Disconnect edge",
                "接続線を削除",
                "断开连线",
                true,
                apply(json!({"kind":"disconnect","id":id}))
            );
        }
        ContextTarget::Stage => {
            add!(
                "Add number here",
                "ここに数値を追加",
                "在此添加数值",
                true,
                json!({"kind":"add","sum":false,"point":context.world,"revision":rev})
            );
            add!(
                "Add sum here",
                "ここに加算を追加",
                "在此添加加法",
                true,
                json!({"kind":"add","sum":true,"point":context.world,"revision":rev})
            );
            add!(
                "Create group from selection…",
                "選択からグループを作成…",
                "从选择创建组…",
                context.selection_count > 0,
                json!({"kind":"group_name","revision":rev})
            );
            add!(
                "Clear selection",
                "選択を解除",
                "清除选择",
                context.selection_count > 0,
                select(vec![])
            );
            menu.append(&PredefinedMenuItem::separator(app)?)?;
            add!(
                "Undo",
                "元に戻す",
                "撤销",
                true,
                request(json!({"kind":"undo","expected_revision":rev}))
            );
            add!(
                "Redo",
                "やり直す",
                "重做",
                true,
                request(json!({"kind":"redo","expected_revision":rev}))
            );
            let active_run = app
                .state::<unge_tauri::Engine>()
                .current_execution("controls")
                .ok()
                .flatten()
                .filter(|run| {
                    matches!(
                        run.state,
                        unge_executor::RunState::Queued | unge_executor::RunState::Running
                    )
                });
            add!(
                "Run graph",
                "グラフを実行",
                "运行节点图",
                active_run.is_none(),
                control("run")
            );
            add!(
                "Cancel run",
                "実行を中止",
                "取消运行",
                active_run.is_some_and(|run| !run.cancel_requested),
                control("cancelRun")
            );
            add!(
                "Reset zoom",
                "ズームをリセット",
                "重置缩放",
                true,
                request(
                    json!({"kind":"set_viewport","viewport":{"zoom":1,"origin":[0,0],"size":context.viewport.size}})
                )
            );
            for (title, entries) in [
                (
                    tr("Appearance", "表示モード", "外观"),
                    vec![
                        (tr("Dark", "ダーク", "深色"), control("themeDark")),
                        (tr("Light", "ライト", "浅色"), control("themeLight")),
                    ],
                ),
                (
                    tr("Language", "言語", "语言"),
                    vec![
                        ("English", json!({"kind":"locale","value":"en"})),
                        ("日本語", json!({"kind":"locale","value":"ja"})),
                        ("简体中文", json!({"kind":"locale","value":"zh-CN"})),
                    ],
                ),
                (
                    tr("Workspace", "ワークスペース", "工作区"),
                    vec![
                        (
                            tr("Zoom…", "ズーム…", "缩放…"),
                            json!({"kind":"zoom","viewport":context.viewport}),
                        ),
                        (
                            tr("Node list…", "ノード一覧…", "节点列表…"),
                            json!({"kind":"browse","target":"nodeChoice"}),
                        ),
                        (
                            tr("Group list…", "グループ一覧…", "组列表…"),
                            json!({"kind":"browse","target":"groupChoice"}),
                        ),
                        (
                            tr(
                                "Float / dock settings",
                                "設定のフロート／ドッキング",
                                "浮动或停靠设置",
                            ),
                            control("panelToggle"),
                        ),
                        (
                            tr("Execution status…", "実行状況…", "执行状态…"),
                            json!({"kind":"browse","target":"run"}),
                        ),
                        (
                            tr("Canvas controls…", "キャンバス操作…", "画布操作…"),
                            json!({"kind":"browse","target":"help"}),
                        ),
                    ],
                ),
            ] {
                let submenu = Submenu::new(app, title, true)?;
                for (label, action) in entries {
                    submenu.append(&item(label, true, action)?)?;
                }
                menu.append(&submenu)?;
            }
        }
    }
    if let Ok(mut slot) = state.items.lock() {
        *slot = actions;
    }
    window.popup_menu_at(&menu, tauri::LogicalPosition::new(position[0], position[1]))
}
