use crate::{
    execute::{CodeExecutor, ExecutionHandle, ExecutionState, ProcessStatus},
    markdown::elements::Code,
    presentation::{AsRenderOperations, PreformattedLine, RenderOnDemand, RenderOnDemandState, RenderOperation},
    render::properties::WindowSize,
    style::Colors,
};
use itertools::Itertools;
use std::{cell::RefCell, rc::Rc};

use super::separator::RenderSeparator;

#[derive(Debug)]
struct RunCodeOperationInner {
    handle: Option<ExecutionHandle>,
    output_lines: Vec<String>,
    state: RenderOnDemandState,
}

#[derive(Debug)]
pub(crate) struct RunCodeOperation {
    code: Code,
    executor: Rc<CodeExecutor>,
    default_colors: Colors,
    block_colors: Colors,
    inner: Rc<RefCell<RunCodeOperationInner>>,
    state_description: RefCell<&'static str>,
}

impl RunCodeOperation {
    pub(crate) fn new(code: Code, executor: Rc<CodeExecutor>, default_colors: Colors, block_colors: Colors) -> Self {
        let inner =
            RunCodeOperationInner { handle: None, output_lines: Vec::new(), state: RenderOnDemandState::default() };
        Self {
            code,
            executor,
            default_colors,
            block_colors,
            inner: Rc::new(RefCell::new(inner)),
            state_description: "running".into(),
        }
    }

    fn render_line(&self, mut line: String) -> RenderOperation {
        if line.contains('\t') {
            line = line.replace('\t', "    ");
        }
        let line_len = line.len() as u16;
        RenderOperation::RenderPreformattedLine(PreformattedLine {
            text: line,
            unformatted_length: line_len,
            block_length: line_len,
            alignment: Default::default(),
        })
    }
}

impl AsRenderOperations for RunCodeOperation {
    fn as_render_operations(&self, dimensions: &WindowSize) -> Vec<RenderOperation> {
        let inner = self.inner.borrow();
        if matches!(inner.state, RenderOnDemandState::NotStarted) {
            return Vec::new();
        }
        let description = self.state_description.borrow();
        let heading = format!(" [{description}] ");
        let separator = RenderSeparator::new(heading);
        let mut operations = vec![
            RenderOperation::RenderLineBreak,
            RenderOperation::RenderDynamic(Rc::new(separator)),
            RenderOperation::RenderLineBreak,
            RenderOperation::RenderLineBreak,
            RenderOperation::SetColors(self.block_colors.clone()),
        ];

        for line in &inner.output_lines {
            let chunks = line.chars().chunks(dimensions.columns as usize);
            for chunk in &chunks {
                operations.push(self.render_line(chunk.collect()));
                operations.push(RenderOperation::RenderLineBreak);
            }
        }
        operations.push(RenderOperation::SetColors(self.default_colors.clone()));
        operations
    }

    fn diffable_content(&self) -> Option<&str> {
        None
    }
}

impl RenderOnDemand for RunCodeOperation {
    fn poll_state(&self) -> RenderOnDemandState {
        let mut inner = self.inner.borrow_mut();
        if let Some(handle) = inner.handle.as_mut() {
            let state = handle.state();
            let ExecutionState { output, status } = state;
            *self.state_description.borrow_mut() = match status {
                ProcessStatus::Running => "running",
                ProcessStatus::Success => "finished",
                ProcessStatus::Failure => "finished with error",
            };
            if status.is_finished() {
                inner.handle.take();
                inner.state = RenderOnDemandState::Rendered;
            }
            inner.output_lines = output;
        }
        inner.state.clone()
    }

    fn start_render(&self) -> bool {
        let mut inner = self.inner.borrow_mut();
        if !matches!(inner.state, RenderOnDemandState::NotStarted) {
            return false;
        }
        match self.executor.execute(&self.code) {
            Ok(handle) => {
                inner.handle = Some(handle);
                inner.state = RenderOnDemandState::Rendering;
                true
            }
            Err(e) => {
                inner.output_lines = vec![e.to_string()];
                inner.state = RenderOnDemandState::Rendered;
                true
            }
        }
    }
}
