// HULA PATCH: Fatal nesting guard; no panic, raw pointer, or borrowed parser guard.
use std::{cell::Cell, rc::Rc};

use crate::{ParserConfig, ParserImpl, diagnostics};

pub(crate) struct NestingGuard(Option<Rc<Cell<usize>>>);

impl Drop for NestingGuard {
    fn drop(&mut self) {
        if let Some(depth) = &self.0 {
            depth.set(depth.get() - 1);
        }
    }
}

impl<C: ParserConfig> ParserImpl<'_, C> {
    #[inline]
    pub(crate) fn enter_nesting(&mut self) -> Option<NestingGuard> {
        self.enter_nesting_at(self.cur_token().span())
    }

    #[inline]
    fn enter_nesting_at(&mut self, span: oxc_span::Span) -> Option<NestingGuard> {
        if self.depth_exceeded.is_some() {
            return None;
        }
        let Some(maximum) = self.options.max_nesting_depth else {
            return Some(NestingGuard(None));
        };
        let depth = self.nesting_depth.get();
        if depth >= maximum {
            self.depth_exceeded = Some(span.start);
            self.set_fatal_error(diagnostics::nesting_depth_exceeded(span));
            return None;
        }
        self.nesting_depth.set(depth + 1);
        Some(NestingGuard(Some(Rc::clone(&self.nesting_depth))))
    }

    // HULA PATCH: Bound external regex recursion before entering the unmodified dependency.
    #[cfg(feature = "regular_expression")]
    pub(crate) fn check_regex_nesting(
        &mut self,
        pattern: &str,
        unicode_sets: bool,
        span: oxc_span::Span,
    ) -> bool {
        if self.options.max_nesting_depth.is_none() {
            return true;
        }
        let mut frames = Vec::new();
        let mut classes = 0;
        let mut escaped = false;
        for byte in pattern.bytes() {
            if escaped {
                escaped = false;
                continue;
            }
            if byte == b'\\' {
                escaped = true;
                continue;
            }
            if (byte == b'(' && classes == 0) || (byte == b'[' && (classes == 0 || unicode_sets)) {
                // Four counter units cover the external parser's larger recursive frames.
                let guards: [Option<NestingGuard>; 4] =
                    std::array::from_fn(|_| self.enter_nesting_at(span));
                if guards.iter().any(Option::is_none) {
                    return false;
                }
                frames.push((byte, guards));
                if byte == b'[' {
                    classes += 1;
                }
            } else if (byte == b')'
                && classes == 0
                && frames.last().is_some_and(|(kind, _)| *kind == b'('))
                || (byte == b']' && classes > 0)
            {
                let (kind, _) = frames.pop().unwrap();
                if kind == b'[' {
                    classes -= 1;
                }
            }
        }
        true
    }
}
