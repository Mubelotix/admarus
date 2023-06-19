use crate::prelude::*;

pub fn wndw() -> web_sys::Window {
    unsafe {
        web_sys::window().unwrap_unchecked()
    }
}

pub async fn sleep(duration: Duration) {
    let promise = Promise::new(&mut |resolve, _| {
        wndw().set_timeout_with_callback_and_timeout_and_arguments_0(
            &resolve,
            duration.as_millis() as i32,
        ).unwrap();
    });
    JsFuture::from(promise).await.unwrap();
}

const LEAK_MEMORY: bool = false;

pub trait HackTraitAnimation<COMP: Component> {
    fn animate_message_owned<T: Into<COMP::Message> + Clone + 'static>(self, msg: T);
    fn animate_message<T: Into<COMP::Message> + Clone + 'static>(&self, msg: T);
    fn animate_callback<F: Fn(IN) -> M + 'static, IN, M: Into<COMP::Message> + Clone + 'static>(&self, function: F) -> Box<dyn Fn(IN)>;
}

impl<COMP: Component> HackTraitAnimation<COMP> for Scope<COMP> {
    fn animate_message_owned<T: Into<COMP::Message> + Clone + 'static>(self, msg: T) {
        let document = unsafe {
            web_sys::window().unwrap_unchecked().document().unwrap_unchecked()
        };
        let start_view_transition = get(&document, &"startViewTransition".into()).unwrap();
        let start_view_transition = match start_view_transition.dyn_ref::<Function>() {
            Some(f) => f.clone(),
            None => {
                log!("startViewTransition is not a function");
                self.send_message(msg);
                return;
            }
        };
        let callback = Closure::wrap(Box::new(move |_: JsValue| {
            self.send_message(msg.clone());
            log!("it works!");
        }) as Box<dyn FnMut(_)>);
        let args = Array::new_with_length(1);
        args.set(0, callback.as_ref().clone());
        apply(&start_view_transition, &document, &args).unwrap();

        if !LEAK_MEMORY {
            spawn_local(async move {
                sleep(Duration::from_secs(5)).await;
                drop(callback)
            })
        } else {
            callback.forget();
        }
    }

    fn animate_message<T: Into<<COMP as Component>::Message> + Clone + 'static>(&self, msg: T) {
        let self2 = self.clone();
        self2.animate_message_owned(msg)
    }

    fn animate_callback<F: Fn(IN) -> M + 'static, IN, M: Into<<COMP as Component>::Message> + Clone + 'static>(&self, function: F) -> Box<dyn Fn(IN)> {
        let self2 = self.clone();
        Box::new(move |i: IN| {
            let message = function(i);
            self2.animate_message(message);
        })
    }
}
