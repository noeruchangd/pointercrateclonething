use futures::StreamExt;
use sqlx::{query, PgConnection};
use std::collections::HashMap;

#[derive(Debug)]
pub struct HeatMap {
    map: HashMap<String, i64>,
}

macro_rules! heatmap_query {
    ($connection:ident, $query:expr) => {{
        let mut heatmap = HashMap::new();

        let mut stream = query!($query).fetch($connection);

        while let Some(row) = stream.next().await {
            let row = row?;

            heatmap.insert(row.iso_country_code, row.value as i64);
        }

        Ok(HeatMap { map: heatmap })
    }};
}

impl HeatMap {
    pub async fn load_total_point_heatmap(connection: &mut PgConnection) -> Result<HeatMap, sqlx::Error> {
        heatmap_query!(
            connection,
            r#"select iso_country_code as "iso_country_code!", sum(score) as "value!" from players_with_score where iso_country_code is not null and score != 0 group by iso_country_code"#
        )
    }

    pub async fn load_total_players_heatmap(connection: &mut PgConnection) -> Result<HeatMap, sqlx::Error> {
        heatmap_query!(
            connection,
            r#"select iso_country_code as "iso_country_code!", count(*) as "value!" from players_with_score where iso_country_code is not null and score != 0 group by iso_country_code"#
        )
    }

    pub fn compute_levels(&self, low_level: i64, high_level: i64) -> HashMap<&String, i64> {
        let sorted_values: Vec<i64> = {
            let mut values: Vec<i64> = self.map.values().map(|v| *v).collect();
            values.sort();
            values
        };

        let mut differences: Vec<(usize, i64)> = sorted_values.windows(2).map(|w| w[1] - w[0]).enumerate().collect();

        differences.sort();

        // search for local maxima in the data stream
        let mut division_points: Vec<usize> = differences
            .windows(3)
            .filter_map(|w| {
                if w[1].1 > w[0].1 && w[2].1 < w[1].1 {
                    Some(w[1].0 + 1)
                } else {
                    None
                }
            })
            .collect();

        if differences.len() > 1 && differences[0] > differences[1] {
            division_points.insert(0, 1);
        }

        let subdivisions = division_points.len() as i64 + 1;

        division_points.insert(0, 0);
        division_points.push(sorted_values.len());

        division_points.sort();

        let max_per_division: Vec<i64> = division_points.iter().skip(1).map(|&idx| sorted_values[idx - 1]).collect();
        let levels_per_subdivision = (high_level - low_level) / subdivisions;

        let mut level_map = HashMap::new();

        for (key, value) in &self.map {
            let rank = sorted_values.iter().position(|v| *v == *value).unwrap();
            let division = division_points.iter().rposition(|&idx| rank >= idx).unwrap();

            let base_level = low_level + ((high_level - low_level) / subdivisions) * (division as i64);

            let level = base_level + *value * levels_per_subdivision / max_per_division[division];

            level_map.insert(key, level);

            /*println!(
                "{} with total score {} at index {}, putting it at division {} with base level {} (highest in subdivision: {}). Thus its \
                 level is {}",
                key, value, rank, division, base_level, max_per_division[division], level
            );*/
        }

        level_map
    }
}
