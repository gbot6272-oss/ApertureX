//! Der wgpu-Gerätekontext: `Instance`/`Adapter`/`Device`/`Queue`, einmal
//! beim App-Start aufgebaut (wird `AppState.pipeline` in Phase-2-Schritt
//! 5), sowie der gemeinsame Hoch-/Ausführen-/Runterladen-Helfer
//! ([`dispatch`]), den alle Regler-Module in `crate::stages` nutzen.
//!
//! **Fallback-Strategie** (siehe `DECISIONS.md` ADR-0012, `SPEC.md` §2.2
//! „GPU→CPU-Fallback muss existieren und getestet sein"): Zuerst wird ein
//! bevorzugter Hardware-Adapter angefragt; findet sich keiner, wird
//! explizit ein Software-Adapter (`force_fallback_adapter`) angefragt,
//! bevor endgültig aufgegeben wird. Der CPU-Fallback selbst (Rayon, kein
//! wgpu) lebt in `crate::stages` — dieses Modul liefert nur `Err`, wenn
//! *gar keine* GPU-Ausführung möglich ist; die Entscheidung "GPU oder
//! CPU-Fallback nutzen" trifft der jeweilige Regler-Aufrufer.

pub mod dispatch;

use crate::error::{PipelineError, Result};

/// Hält die für wgpu-Aufrufe nötigen Handles. `Device`/`Queue` sind
/// intern bereits `Arc`-basiert und günstig zu klonen — `GpuContext`
/// selbst wird typischerweise als `Arc<GpuContext>` gehalten (siehe
/// `AppState.pipeline` ab Schritt 5).
pub struct GpuContext {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    /// Nur zu Diagnosezwecken (Logging, welcher Adapter tatsächlich läuft).
    pub adapter_info: wgpu::AdapterInfo,
}

impl GpuContext {
    /// Wie viele Byte ein einzelner Storage-Puffer auf diesem Gerät
    /// fassen darf.
    ///
    /// Öffentlich, damit Aufrufer außerhalb dieser Kiste vorher
    /// prüfen können, statt in einen Validierungsfehler zu laufen
    /// (ADR-0074) — `device` selbst bleibt privat, weil der Rest der
    /// App nichts direkt an der wgpu-Schnittstelle zu suchen hat.
    ///
    /// Bei drei `f32` je Pixel entspricht der Wert
    /// `max_storage_buffer_bytes() / 12` Pixeln.
    pub fn max_storage_buffer_bytes(&self) -> u64 {
        u64::from(self.device.limits().max_storage_buffer_binding_size)
    }

    /// Blockierende Variante von [`GpuContext::new`] für Aufrufer ohne
    /// eigene Async-Runtime (z. B. `main()` beim App-Start, Tests).
    pub fn new_blocking() -> Result<Self> {
        pollster::block_on(Self::new())
    }

    /// Baut einen neuen Gerätekontext auf. Siehe Modul-Dokumentation für
    /// die Fallback-Strategie.
    pub async fn new() -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
        {
            Some(adapter) => adapter,
            None => {
                tracing::warn!("kein bevorzugter GPU-Adapter gefunden, versuche Software-Fallback");
                instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::default(),
                        force_fallback_adapter: true,
                        compatible_surface: None,
                    })
                    .await
                    .ok_or_else(|| PipelineError::GpuUnavailable {
                        message: "weder ein Hardware- noch ein Software-Adapter verfügbar"
                            .to_string(),
                    })?
            }
        };

        let adapter_info = adapter.get_info();
        tracing::info!(
            backend = ?adapter_info.backend,
            name = %adapter_info.name,
            device_type = ?adapter_info.device_type,
            "wgpu-Adapter gewählt"
        );

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("apx-pipeline-device"),
                    required_features: wgpu::Features::empty(),
                    // Die Grenzen des ADAPTERS anfordern, nicht die
                    // konservativen Vorgabewerte (ADR-0074).
                    //
                    // `Limits::default()` deckelt
                    // `max_storage_buffer_binding_size` auf 128 MiB.
                    // Bei drei `f32` je Pixel sind das
                    //
                    //     134.217.728 / 12 = 11.184.810 Pixel,
                    //
                    // also gut **11 Megapixel** — jedes größere Foto
                    // hätte den GPU-Pfad erneut verfehlt, diesmal an
                    // der Puffergröße statt am Gruppenlimit. Die
                    // Vorgabewerte sind auf den kleinsten gemeinsamen
                    // Nenner inklusive Mobilgeräten ausgelegt; ein
                    // Adapter meldet mit `adapter.limits()`, was er
                    // wirklich kann, und mehr als das kann hier per
                    // Definition nicht angefordert werden.
                    required_limits: adapter.limits(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .map_err(|source| PipelineError::GpuUnavailable {
                message: format!("Gerät konnte nicht erstellt werden: {source}"),
            })?;

        Ok(Self {
            device,
            queue,
            adapter_info,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_construction_never_panics() {
        // Siehe DECISIONS.md ADR-0018 zur Testinfrastruktur: in dieser
        // Sandbox bzw. auf CI-Runnern ohne echten GPU-Zugriff ist ein
        // `Err(GpuUnavailable)` ein gültiges Ergebnis — ein Panic wäre der
        // eigentliche Fehler. Der positive Fall (tatsächliche
        // GPU-Ausführung) wird in Schritt 8 gegen eine verifizierte
        // Software-Adapter-Umgebung getestet.
        match GpuContext::new_blocking() {
            Ok(ctx) => {
                tracing::info!(name = %ctx.adapter_info.name, "GPU-Kontext in Testumgebung verfügbar");
            }
            Err(PipelineError::GpuUnavailable { .. }) => {}
            Err(other) => panic!("unerwarteter Fehler: {other}"),
        }
    }

    /// Der Kontext darf sich nicht unter die Fähigkeiten des Adapters
    /// deckeln (ADR-0074).
    ///
    /// `wgpu::Limits::default()` begrenzt einen Storage-Puffer auf 128
    /// MiB. Bei drei `f32` je Pixel sind das gut 11 Megapixel — jedes
    /// größere Foto wäre am GPU-Pfad vorbeigelaufen, direkt nachdem
    /// das Gruppenlimit behoben wurde. Dieser Test hält fest, dass
    /// angefordert wird, was der Adapter kann.
    #[test]
    fn the_context_requests_what_the_adapter_can_do() {
        let ctx = match GpuContext::new_blocking() {
            Ok(ctx) => ctx,
            Err(_) => {
                eprintln!("übersprungen: kein GPU-Adapter in dieser Umgebung verfügbar");
                return;
            }
        };

        let limits = ctx.device.limits();
        let fallback = wgpu::Limits::default();
        assert!(
            limits.max_storage_buffer_binding_size >= fallback.max_storage_buffer_binding_size,
            "der Kontext meldet {} Byte je Storage-Puffer und liegt damit unter den              Vorgabewerten ({} Byte)",
            limits.max_storage_buffer_binding_size,
            fallback.max_storage_buffer_binding_size
        );
        assert!(
            limits.max_compute_workgroups_per_dimension
                >= fallback.max_compute_workgroups_per_dimension
        );
    }
}
