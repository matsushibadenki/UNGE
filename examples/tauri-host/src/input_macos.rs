//! App-local magnification events. Tao 0.37 exposes precise scroll but no pinch.
use block2::RcBlock;
use objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSEvent, NSEventMask};
use std::{
    cell::RefCell,
    ptr::NonNull,
    sync::{Arc, Mutex},
};
use tauri::EventLoopMessage;
use tauri_runtime_wry::{Message, tao::event_loop::EventLoopProxy};

#[derive(Clone, Copy)]
pub(super) struct Pinch {
    pub position: [f64; 2], // AppKit content-view points, top-left origin
    pub magnification: f64,
}
pub(super) type Pending = Arc<Mutex<Vec<Pinch>>>;

thread_local! {
    static MONITOR: RefCell<Option<Retained<AnyObject>>> = const { RefCell::new(None) };
}

pub(super) fn remove() {
    MONITOR.with(|slot| {
        if let Some(token) = slot.take() {
            // The token was returned by AppKit on this same main thread.
            unsafe { NSEvent::removeMonitor(&token) };
        }
    });
}

pub(super) fn install(
    canvas: &tauri::Window,
    pending: Pending,
    proxy: EventLoopProxy<Message<EventLoopMessage>>,
) -> bool {
    let Some(main) = MainThreadMarker::new() else {
        return false;
    };
    let Ok(window) = canvas.ns_window() else {
        return false;
    };
    let identity = window as usize;
    remove();
    let handler = RcBlock::new(move |raw: NonNull<NSEvent>| {
        // AppKit owns the event for the duration of the local monitor callback.
        let event = unsafe { raw.as_ref() };
        if let Some(window) = event.window(main)
            && Retained::as_ptr(&window) as usize == identity
            && let Some(view) = window.contentView()
        {
            let p = view.convertPoint_fromView(event.locationInWindow(), None);
            let bounds = view.bounds();
            let x = p.x - bounds.origin.x;
            let y = p.y - bounds.origin.y;
            let y = if view.isFlipped() {
                y
            } else {
                bounds.size.height - y
            };
            let magnification = event.magnification();
            if magnification.is_finite()
                && magnification != 0.
                && let Ok(mut events) = pending.lock()
                && events.len() < 64
            {
                let wake = events.is_empty();
                events.push(Pinch {
                    position: [x, y],
                    magnification,
                });
                if wake {
                    let _ = proxy.send_event(Message::Task(Box::new(|| {})));
                }
            }
        }
        raw.as_ptr() // Preserve normal AppKit/WebView handling.
    });
    // The handler always returns AppKit's original valid event pointer.
    let token = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::Magnify, &handler)
    };
    let installed = token.is_some();
    MONITOR.with(|slot| slot.replace(token));
    installed
}
