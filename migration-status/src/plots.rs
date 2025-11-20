use chrono::{DateTime, Datelike, Utc};
use plotters::chart::SeriesLabelPosition;
use plotters::prelude::*;
use tokei::LanguageType;

use crate::analyze;

pub fn plot_language_division(
    result: &analyze::TokeiStatistics,
    owner: &str,
    repo: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if result.steps.is_empty() {
        return Ok(());
    }

    let file_name = format!("results/{}_{}_division.png", owner, repo);
    let root = BitMapBackend::new(&file_name, (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut times: Vec<DateTime<Utc>> = result.steps.iter().map(|s| s.commit_date).collect();
    times.sort();

    let start = *times.first().unwrap();
    let end = *times.last().unwrap();

    let mut chart = ChartBuilder::on(&root)
        .caption(format!("Languages used over time for {}/{}", owner, repo), ("sans-serif", 20))
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(start..end, 0.0f64..1.0f64)?;

    chart
        .configure_mesh()
        .x_desc("Year")
        .y_desc("Percentage")
        .x_labels((end.year() - start.year() + 1) as usize)
        .x_label_formatter(&|dt: &DateTime<Utc>| format!("{}", dt.year()))
        .draw()?;

    // Collect all languages that ever appeared
    let mut languages: Vec<LanguageType> = result
        .steps
        .iter()
        .flat_map(|step| step.languages.keys().cloned())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .filter(|lang| analyze::is_code_language(lang))
        .collect();

    languages.sort_by(|a, b| a.name().cmp(b.name()));

    let step_count = result.steps.len();

    // Accumulator for stacked percentages per step
    let mut accumulator: Vec<f64> = vec![0.0; step_count];

    for (idx, lang) in languages.iter().enumerate() {
        let color = Palette99::pick(idx);

        // For each language, draw a stacked "band" over time
        chart
            .draw_series(result.steps.iter().enumerate().map(|(step_idx, step)| {
                let total: usize = step.languages.iter().filter_map(|(l, v)| if analyze::is_code_language(l) { Some(v.code) } else { None }).sum();
                let count = step.languages.get(lang).map_or(0, |info| info.code);

                let percentage = if total > 0 {
                    count as f64 / total as f64
                } else {
                    0.0
                };

                let y0 = accumulator[step_idx];
                let y1 = y0 + percentage;
                accumulator[step_idx] = y1;

                let x0 = times[step_idx];
                let x1 = if step_idx + 1 < step_count {
                    times[step_idx + 1]
                } else {
                    end
                };

                Rectangle::new([(x0, y0), (x1, y1)], color.filled())
            }))?
            .label(format!("{:?}", lang))
            .legend(move |(x, y)| {
                Rectangle::new(
                    [(x, y - 5), (x + 10, y + 5)],
                    Palette99::pick(idx).filled(),
                )
            });
    }

    chart
        .configure_series_labels()
        .border_style(&BLACK)
        .background_style(&WHITE.mix(0.8))
        .position(SeriesLabelPosition::LowerLeft)
        .draw()?;

    root.present()?;
    Ok(())
}

pub fn plot_toml_amount(
    result: &analyze::CargoStatistics,
    owner: &str,
    repo: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if result.steps.is_empty() {
        return Ok(());
    }

    let file_name = format!("results/{}_{}_toml_files.png", owner, repo);
    let root = BitMapBackend::new(&file_name, (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut times: Vec<DateTime<Utc>> = result.steps.iter().map(|s| s.commit_date).collect();
    times.sort();

    let start = *times.first().unwrap();
    let end = *times.last().unwrap();

    let max_files = result.steps.iter().map(|s| s.num_toml_files).max().unwrap_or(0);
    let max_y = if max_files == 0 { 1 } else { max_files };

    let mut chart = ChartBuilder::on(&root)
        .caption(format!("TOML files in repo {}/{} over time", owner, repo), ("sans-serif", 20))
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_cartesian_2d(start..end, 0..max_y)?;

    chart
        .configure_mesh()
        .x_desc("Year")
        .y_desc("Number of TOML Files")
        .x_labels((end.year() - start.year() + 1) as usize)
        .x_label_formatter(&|dt: &DateTime<Utc>| format!("{}", dt.year()))
        .draw()?;

    chart.draw_series(LineSeries::new(
        result.steps.iter().map(|step| (step.commit_date, step.num_toml_files)),
        &RED,
    ))?
    .label("TOML Files")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 10, y)], &RED));

    chart
        .configure_series_labels()
        .border_style(&BLACK)
        .background_style(&WHITE.mix(0.8))
        .position(SeriesLabelPosition::LowerLeft)
        .draw()?;

    root.present()?;
    Ok(())
}