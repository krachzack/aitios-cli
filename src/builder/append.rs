use spec::{BenchSpec, SimulationSpec};
use std::path::PathBuf;

pub fn append(first: SimulationSpec, second: &SimulationSpec) -> SimulationSpec {
    if first.iterations.is_some()
        && second.iterations.is_some()
        && first.iterations.unwrap() != second.iterations.unwrap()
    {
        warn!(
            "Merging simulations specs, one has {first} iterations, but the second has {second} iterations. Using {second} for combined iterations",
            first = first.iterations.unwrap(),
            second = second.iterations.unwrap(),
        )
    }

    SimulationSpec {
        name: append_textual(&first.name, &second.name, "-"),
        description: append_textual(&first.description, &second.description, "\n\n"),
        scenes: append_list(first.scenes, second.scenes.iter()),
        iterations: second.iterations.or(first.iterations),
        effect_interval: second.effect_interval.or(first.effect_interval),
        log: append_log(first.log, &second.log),
        surfel_distance: append_surfel_distance(first.surfel_distance, second.surfel_distance),
        sources: append_list(first.sources, &second.sources),
        surfels_by_material: {
            let mut first = first.surfels_by_material;
            let second = &second.surfels_by_material;
            first.extend(second.clone().into_iter());
            first
        },
        effects: append_list(first.effects, second.effects.iter()),
        benchmark: append_benchmark(&first.benchmark, &second.benchmark),
        consistent_transport: second.consistent_transport.or(first.consistent_transport),
    }
}

fn append_surfel_distance(first: Option<f32>, second: Option<f32>) -> Option<f32> {
    match (first, second) {
        (Some(first), Some(second)) => {
            if first != second {
                warn!(
                    "Conflicting surfel distances from simulation fragments: {first} and {second}. Using {second}."
                    ,first = first, second = second
                );
            }
            Some(second)
        }
        (first, second) => second.or(first),
    }
}

fn append_textual(first: &str, second: &str, delimiter: &str) -> String {
    match (first.trim(), second.trim()) {
        ("", "") => String::new(),
        (first, "") => String::from(first),
        ("", second) => String::from(second),
        // Concatenate non-empty strings with dashes
        (first, second) => format!("{}{}{}", first, delimiter, second),
    }
}

fn append_list<'a, T, I>(mut first: Vec<T>, second: I) -> Vec<T>
where
    I: IntoIterator<Item = &'a T>,
    T: 'a + Clone,
{
    first.extend(second.into_iter().cloned());
    first
}

/*fn append_map<K,V>(mut first: HashMap<K,V>, second: &HashMap<K,V>) -> HashMap<K,V>
where K : Eq+ Hash+Debug, V:Eq+Clone+Debug {
    for (second_key, second_value) in second.iter() {
        let new_val = match (first.get(second_key), second_value) {
            // Same value, no update necessary.
            (Some(val1), val2) if val1 == val2 => val1.clone(),
            // First does not have what second has, merge it.
            (None, val) => val.clone(),
            // Conflicting values, print a warnign and use value of second.
            (Some(val1), val2) => {
                warn!("Merging simulation spec file and {key:?} has value {val1:?} in one and {val2:?} in the other. Using {val2:?} in merged spec.",
                    key = second_key,
                    val1 = val1,
                    val2 = val2,
                );
                val2.clone()
            },
        };

        first[second_key] = new_val;
    }
    first.extend(second.iter());
    first
}*/

fn append_log(first: Option<PathBuf>, second: &Option<PathBuf>) -> Option<PathBuf> {
    match (first, second.as_ref()) {
        (Some(first), Some(second)) => {
            if &first == second {
                // If equal, no problem
                Some(first)
            } else {
                // Two different log files smells fishy
                warn!("Merging simulation specs and two different log files defined: {first:?}, {second:?}. Removing {first:?} and using only {second:?}!",
                    first = first,
                    second = second);
                Some(PathBuf::from(second))
            }
        }
        // Use second if specified, otherwise use first
        (first, second) => second.map(PathBuf::from).or(first),
    }
}

fn append_benchmark(first: &Option<BenchSpec>, second: &Option<BenchSpec>) -> Option<BenchSpec> {
    fn second_or_first(first: &Option<PathBuf>, second: &Option<PathBuf>) -> Option<PathBuf> {
        second.as_ref().or(first.as_ref()).map(PathBuf::from)
    }

    match (first, second) {
        (Some(first), Some(second)) => Some(BenchSpec {
            iterations: second_or_first(&first.iterations, &second.iterations),
            tracing: second_or_first(&first.tracing, &second.tracing),
            synthesis: second_or_first(&first.synthesis, &second.synthesis),
            setup: second_or_first(&first.setup, &second.setup),
        }),
        (Some(spec), None) => Some(spec.clone()),
        (None, Some(spec)) => Some(spec.clone()),
        _ => None,
    }
}
