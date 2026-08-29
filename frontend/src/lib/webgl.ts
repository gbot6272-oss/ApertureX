/**
 * Minimaler WebGL2-Renderer für den Viewer: eine texturierte Quad-Fläche,
 * deren Position/Größe über eine einfache 2D-Transformation (Ursprung in
 * Geräte-Pixeln + Zielgröße) gesteuert wird — dieselbe Geometrie, die
 * vorher direkt an `ctx.drawImage(...)` ging (siehe `lib/viewerMath.ts`,
 * unverändert wiederverwendet).
 *
 * Wechsel von Canvas-2D auf WebGL2 laut `PLAN.md` Phase 2 Schritt 6:
 * WebGL2 kann sowohl ein dekodiertes `ImageBitmap` (bestehender
 * Vorschau-/Vollbild-Pfad) als auch einen rohen RGBA8-Puffer (neue
 * Entwickeln-Route, `hooks/useDevelopRender`) als Textur hochladen —
 * Canvas-2D bräuchte für Letzteres einen Umweg über `ImageData`.
 */

const VERTEX_SHADER = `#version 300 es
in vec2 a_position;
in vec2 a_texCoord;
uniform vec2 u_canvasSize;
uniform vec2 u_origin;
uniform vec2 u_imageSize;
out vec2 v_texCoord;

void main() {
  vec2 pixelPos = u_origin + a_position * u_imageSize;
  vec2 clipSpace = (pixelPos / u_canvasSize) * 2.0 - 1.0;
  gl_Position = vec4(clipSpace.x, -clipSpace.y, 0.0, 1.0);
  v_texCoord = a_texCoord;
}
`;

const FRAGMENT_SHADER = `#version 300 es
precision mediump float;
in vec2 v_texCoord;
uniform sampler2D u_texture;
out vec4 outColor;

void main() {
  outColor = texture(u_texture, v_texCoord);
}
`;

function compileShader(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error("WebGL-Shader konnte nicht angelegt werden");
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const info = gl.getShaderInfoLog(shader);
    gl.deleteShader(shader);
    throw new Error(`Shader-Kompilierung fehlgeschlagen: ${info ?? "unbekannt"}`);
  }
  return shader;
}

export interface Origin {
  x: number;
  y: number;
}

/** Zeichnet eine einzelne Bild-Textur auf einen WebGL2-Canvas. Eine
 * Instanz pro `<canvas>`-Element, über dessen Lebenszeit wiederverwendet
 * (siehe `dispose()` beim Unmount). */
export class QuadRenderer {
  private readonly gl: WebGL2RenderingContext;
  private readonly program: WebGLProgram;
  private readonly texture: WebGLTexture;
  private readonly vao: WebGLVertexArrayObject;
  private readonly locations: {
    canvasSize: WebGLUniformLocation | null;
    origin: WebGLUniformLocation | null;
    imageSize: WebGLUniformLocation | null;
  };

  constructor(canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl2");
    if (!gl) throw new Error("WebGL2 wird von diesem Browser/Gerät nicht unterstützt");
    this.gl = gl;

    const vertexShader = compileShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER);
    const fragmentShader = compileShader(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER);
    const program = gl.createProgram();
    if (!program) throw new Error("WebGL-Programm konnte nicht angelegt werden");
    gl.attachShader(program, vertexShader);
    gl.attachShader(program, fragmentShader);
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      const info = gl.getProgramInfoLog(program);
      throw new Error(`WebGL-Programm-Verknüpfung fehlgeschlagen: ${info ?? "unbekannt"}`);
    }
    this.program = program;

    // Ein Quad aus zwei Dreiecken, Einheitsquadrat [0,1]x[0,1] —
    // a_position wird im Vertex-Shader mit u_imageSize skaliert und um
    // u_origin verschoben, statt hier pro Bild einen neuen Puffer zu
    // bauen.
    const positions = new Float32Array([0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 1]);
    const texCoords = new Float32Array([0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 1]);

    const vao = gl.createVertexArray();
    if (!vao) throw new Error("WebGL-VAO konnte nicht angelegt werden");
    gl.bindVertexArray(vao);
    this.vao = vao;

    const positionBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, positions, gl.STATIC_DRAW);
    const positionLoc = gl.getAttribLocation(program, "a_position");
    gl.enableVertexAttribArray(positionLoc);
    gl.vertexAttribPointer(positionLoc, 2, gl.FLOAT, false, 0, 0);

    const texCoordBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, texCoordBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, texCoords, gl.STATIC_DRAW);
    const texCoordLoc = gl.getAttribLocation(program, "a_texCoord");
    gl.enableVertexAttribArray(texCoordLoc);
    gl.vertexAttribPointer(texCoordLoc, 2, gl.FLOAT, false, 0, 0);

    const texture = gl.createTexture();
    if (!texture) throw new Error("WebGL-Textur konnte nicht angelegt werden");
    this.texture = texture;
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);

    this.locations = {
      canvasSize: gl.getUniformLocation(program, "u_canvasSize"),
      origin: gl.getUniformLocation(program, "u_origin"),
      imageSize: gl.getUniformLocation(program, "u_imageSize"),
    };
  }

  /** Lädt ein dekodiertes Bitmap als Textur hoch (bestehender
   * Vorschau-/Vollbild-Pfad, siehe `hooks/useImageBitmap`). */
  uploadImageBitmap(bitmap: ImageBitmap): void {
    const gl = this.gl;
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, bitmap);
  }

  /** Lädt einen rohen, interleaved RGBA8-Puffer als Textur hoch (neue
   * Entwickeln-Route, siehe `hooks/useDevelopRender`). */
  uploadRgba8(width: number, height: number, pixels: Uint8Array): void {
    const gl = this.gl;
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
  }

  /** Über 100 % Zoom scharfe Pixelkanten statt weichgezeichneter
   * Vergrößerung (wie zuvor `ctx.imageSmoothingEnabled` in der
   * Canvas-2D-Fassung, siehe `DECISIONS.md`/`PHASE1_PROMPT.md` Abschnitt
   * 7) — `enabled` sollte `effectiveScale <= 1` sein. */
  setSmoothing(enabled: boolean): void {
    const gl = this.gl;
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    const filter = enabled ? gl.LINEAR : gl.NEAREST;
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, filter);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, filter);
  }

  /** Zeichnet die zuletzt hochgeladene Textur. `canvasCssWidth`/
   * `canvasCssHeight` in CSS-Pixeln, `origin`/`imageWidth`/`imageHeight`
   * ebenfalls in CSS-Pixeln (wie zuvor an `ctx.drawImage` übergeben) —
   * die Umrechnung auf Geräte-Pixel via `dpr` passiert hier intern. */
  draw(canvasCssWidth: number, canvasCssHeight: number, dpr: number, origin: Origin, imageWidth: number, imageHeight: number): void {
    const gl = this.gl;
    const canvas = gl.canvas as HTMLCanvasElement;
    const pixelWidth = Math.max(1, Math.round(canvasCssWidth * dpr));
    const pixelHeight = Math.max(1, Math.round(canvasCssHeight * dpr));
    if (canvas.width !== pixelWidth) canvas.width = pixelWidth;
    if (canvas.height !== pixelHeight) canvas.height = pixelHeight;
    canvas.style.width = `${canvasCssWidth}px`;
    canvas.style.height = `${canvasCssHeight}px`;

    gl.viewport(0, 0, pixelWidth, pixelHeight);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);

    if (imageWidth <= 0 || imageHeight <= 0) return;

    gl.useProgram(this.program);
    gl.bindVertexArray(this.vao);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);

    gl.uniform2f(this.locations.canvasSize, pixelWidth, pixelHeight);
    gl.uniform2f(this.locations.origin, origin.x * dpr, origin.y * dpr);
    gl.uniform2f(this.locations.imageSize, imageWidth * dpr, imageHeight * dpr);

    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  }

  dispose(): void {
    const gl = this.gl;
    gl.deleteTexture(this.texture);
    gl.deleteVertexArray(this.vao);
    gl.deleteProgram(this.program);
  }
}
