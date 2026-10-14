#![forbid(unsafe_code)]

//! The set of names a walk has registered.

/// A widget name in the set a walk keeps, so that a repeated one is found in
/// logarithmic time: a control array of a thousand elements must not cost a
/// million comparisons. Tests count the comparisons.
pub(super) struct Name(pub(super) String);

impl PartialEq for Name {
    fn eq(&self, other: &Name) -> bool {
        #[cfg(test)]
        tests::COMPARISONS.with(|count| count.set(count.get() + 1));
        self.0 == other.0
    }
}

impl Eq for Name {}

impl PartialOrd for Name {
    fn partial_cmp(&self, other: &Name) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Name {
    fn cmp(&self, other: &Name) -> std::cmp::Ordering {
        #[cfg(test)]
        tests::COMPARISONS.with(|count| count.set(count.get() + 1));
        self.0.cmp(&other.0)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use xui_canvas::OffscreenBackend;
    use xui_core::app::{App, Ui, run_app};
    use xui_core::backend::{Backend, PlatformSpec};
    use xui_core::units::Dip;

    use crate::build::{BuildOptions, Factories, Handlers, describe};
    use crate::model::{Button, Form, Node, Stack};

    thread_local! {
        /// How many times a walk compared two widget names.
        pub(super) static COMPARISONS: Cell<usize> = const { Cell::new(0) };
    }

    struct Idle;

    impl App for Idle {
        type Msg = ();

        fn update(&mut self, _msg: (), _ui: &mut Ui<()>) {}
    }

    /// The name comparisons describing a control array of `elements` takes.
    fn comparisons(elements: u32) -> usize {
        let form = Form {
            root: Node::Column(Stack {
                children: vec![Node::Button(Button {
                    name: "key".to_owned(),
                    array: Some(elements),
                    ..Button::default()
                })],
                ..Stack::default()
            }),
            ..Form::default()
        };
        COMPARISONS.with(|count| count.set(0));
        let backend: Rc<dyn Backend> = Rc::new(OffscreenBackend::new());
        let spec = PlatformSpec::new("names").size(Dip(100.0), Dip(100.0));
        run_app(backend, spec, move |ui| {
            let described = describe(
                ui,
                &form,
                &Factories::xui(),
                &Handlers::<()>::new(),
                BuildOptions::default(),
            )
            .expect("the form describes");
            assert_eq!(described.controls.names().count(), elements as usize);
            Idle
        })
        .expect("run_app succeeds");
        COMPARISONS.with(Cell::get)
    }

    #[test]
    fn registering_names_does_not_scale_quadratically() {
        let (small, large) = (comparisons(256), comparisons(1024));
        assert!(small >= 255, "the counter sees the names: {small}");
        // Four times the elements is about six times the work (n log n); a
        // scan of the names so far would be sixteen.
        assert!(
            large < 10 * small,
            "{small} comparisons for 256 names, {large} for 1024"
        );
    }
}
