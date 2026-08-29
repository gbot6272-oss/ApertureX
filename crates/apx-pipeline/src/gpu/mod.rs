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
                    required_limits: wgpu::Limits::default(),
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
}
