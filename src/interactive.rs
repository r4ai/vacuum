use std::path::Path;
use anyhow::Context as _;
use bytesize::ByteSize;
use dialoguer::{MultiSelect, theme::ColorfulTheme};

use crate::adapter::CleanTarget;

/// Present an interactive multi-select prompt and return the targets
/// the user chose for deletion.
pub fn select_targets(targets: &[CleanTarget], root: &Path) -> anyhow::Result<Vec<CleanTarget>> {
    if targets.is_empty() {
        return Ok(vec![]);
    }

    let items: Vec<String> = targets
        .iter()
        .map(|t| {
            let rel = t.path.strip_prefix(root).unwrap_or(&t.path);
            format!(
                "[{adapter}]  {path}  ({size})  {desc}",
                adapter = t.adapter,
                path = rel.display(),
                size = ByteSize(t.size),
                desc = t.description,
            )
        })
        .collect();

    // All items start checked (opt-out model)
    let defaults: Vec<bool> = vec![true; items.len()];

    let chosen_indices = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select items to delete  (Space=toggle, Enter=confirm, Esc/q=cancel)")
        .items(&items)
        .defaults(&defaults)
        .interact_opt()
        .context("Interactive selection failed")?;

    match chosen_indices {
        None => Ok(vec![]),
        Some(indices) => Ok(indices.into_iter().map(|i| targets[i].clone()).collect()),
    }
}
