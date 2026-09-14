use super::*;
use crate::EngineMessage;

fn same(actual: Option<&RecordV1<String>>, expected: RecordRef<'_>) -> Result<()> {
    let actual = actual.ok_or_else(|| error("missing sampled publication row"))?;
    let a = serde_json::to_vec(actual).map_err(|e| error(e.to_string()))?;
    struct Compare<'a> {
        bytes: &'a [u8],
        at: usize,
    }
    impl std::io::Write for Compare<'_> {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            if self.bytes.get(self.at..self.at.saturating_add(b.len())) != Some(b) {
                return Err(std::io::Error::other(
                    "sampled host publication disagrees with sim cache",
                ));
            }
            self.at += b.len();
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut compare = Compare { bytes: &a, at: 0 };
    serde_json::to_writer(&mut compare, &expected).map_err(|e| error(e.to_string()))?;
    check(
        compare.at == a.len(),
        "sampled host publication disagrees with sim cache",
    )
}
pub(super) fn validate_publications(
    rows: &[RecordV1<String>],
    s: &ScalarsV1,
    c: HostCheckpointContext<'_>,
) -> Result<()> {
    use RecordV1 as R;
    let now = c.elapsed.as_secs_f64();
    let time = c.clock().at(now);
    check(
        s.clock.day == time.day
            && s.clock.fraction.to_bits() == time.fraction.to_bits()
            && s.clock.office == time.office.into()
            && s.clock.weekday == time.weekday.into()
            && s.clock.brightness.to_bits() == c.clock().brightness(now).to_bits()
            && s.clock.scale.to_bits() == c.clock().scale().to_bits()
            && s.clock.seconds_per_day.to_bits() == c.clock().seconds_per_day().to_bits(),
        "host calendar publication disagreement",
    )?;
    let mut law = rows
        .iter()
        .filter(|r| matches!(r, R::Custody { .. } | R::Holder { .. } | R::Notice { .. }));
    if let Some(EngineMessage::LawStanding { notices, custody }) = c.law() {
        if let Some(p) = custody {
            same(
                law.next(),
                R::Custody {
                    officer: Nullable(p.officer_id.as_ref().map(|id| id.as_str())),
                    officer_name: &p.officer_name,
                    station_name: &p.station_name,
                    anchor: [
                        p.anchor_m.x as f32,
                        p.anchor_m.y as f32,
                        p.anchor_m.z as f32,
                    ],
                    closing: p.closing,
                    strain_seconds: p.strain_seconds as f32,
                    held: p.held,
                    committed: p.committed,
                    fee_sparks: p.fee_sparks,
                    release_office: Nullable(p.release_office.as_deref()),
                    booked_as: Nullable(p.booked_as.as_deref()),
                },
            )?;
            for h in &p.holder_ids {
                same(law.next(), R::Holder { actor: h.as_str() })?;
            }
        }
        for n in notices {
            same(
                law.next(),
                R::Notice {
                    id: n.notice_id,
                    line: &n.line,
                    rung: n.rung.into(),
                    clears_when: &n.clears_when,
                },
            )?;
        }
    }
    check(law.next().is_none(), "extra law publication row")?;
    let mut journal = rows
        .iter()
        .filter(|r| matches!(r, R::Journal { .. } | R::JournalStanding { .. }));
    if let Some(EngineMessage::Journal { entries, standing }) = c.journal() {
        for e in entries {
            let source_bytes = e
                .word
                .len()
                .saturating_add(e.when.len())
                .saturating_add(e.from.as_ref().map_or(0, String::len))
                .saturating_add(e.place.as_ref().map_or(0, String::len));
            check(
                source_bytes <= MAX_TEXT_BYTES,
                "journal formatting source exceeds admitted scratch",
            )?;
            let (attribution, word) = resolved_journal_row(e);
            same(
                journal.next(),
                R::Journal {
                    attribution: &attribution,
                    word: &word,
                },
            )?;
        }
        for text in standing {
            same(journal.next(), R::JournalStanding { text })?;
        }
    }
    check(journal.next().is_none(), "extra journal publication row")?;
    let mut chalk = rows
        .iter()
        .filter(|r| matches!(r, R::ChalkPen { .. } | R::ChalkAnchor { .. }));
    if let Some(EngineMessage::ChalkStanding { pen, anchors }) = c.chalk() {
        same(chalk.next(), R::ChalkPen { present: *pen })?;
        for a in anchors {
            check(a.kinds.len() <= 3, "unsupported chalk publication kinds")?;
            let mut kinds = [Nullable(None); 3];
            for (k, v) in kinds.iter_mut().zip(&a.kinds) {
                k.0 = Some((*v).into());
            }
            same(
                chalk.next(),
                R::ChalkAnchor {
                    handle: &a.handle,
                    label: &a.label,
                    kinds,
                },
            )?;
        }
    } else {
        same(chalk.next(), R::ChalkPen { present: false })?;
    }
    check(chalk.next().is_none(), "extra chalk publication row")
}

/// The ordinary host's exact observer-resolved wording. Keeping the formatter
/// here lets both the UI and admitted candidate validation use the same policy;
/// it takes already-resolved JournalEntry values and never looks up identities.
pub fn resolved_journal_row(entry: &crate::JournalEntry) -> (String, String) {
    let mut parts = Vec::new();
    if let Some(from) = &entry.from {
        parts.push(from.clone());
    }
    if let Some(place) = &entry.place {
        let lower = place.to_lowercase();
        if [
            "in ", "inside ", "at ", "next to ", "near ", "on ", "beside ", "outside ",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
        {
            let mut letters = place.chars();
            parts.push(letters.next().map_or_else(String::new, |first| {
                first.to_lowercase().collect::<String>() + letters.as_str()
            }));
        } else {
            parts.push(format!("at {place}"));
        }
    }
    parts.push(entry.when.clone());
    let register = match entry.hops {
        0 => "you saw it yourself",
        1 => "at one remove",
        2 => "at second hand",
        _ => "at third hand or worse",
    };
    let mut attribution = format!("{} — {register}", parts.join(", "));
    if entry.tellings > 1 {
        attribution.push_str(&format!(
            " — and {} {} since, in {} {}",
            entry.tellings - 1,
            if entry.tellings == 2 {
                "other"
            } else {
                "others"
            },
            entry.wards,
            if entry.wards == 1 { "ward" } else { "wards" }
        ));
    }
    attribution.push(':');
    (attribution, format!("“{}”", entry.word))
}
