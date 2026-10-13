//_DEFINES_

#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif

precision highp float;

#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif

uniform float alpha;
varying vec2 v_coords;

uniform float u_progress;
uniform float u_for_opening;
uniform float u_duration;
uniform vec2 u_size;
uniform vec4 u_color;

const float BLUR_WIDTH = 0.01;
const float TB_TIME    = 0.7;
const float LR_TIME    = 0.4;
const float LR_DELAY   = 0.6;
const float FF_TIME    = 0.1;
const float SCALING    = 0.5;

float easeOutQuad(float x) {
    return -1.0 * x * (x - 2.0);
}

vec4 getInputColor(vec2 coords) {
    if (coords.x < 0.0 || coords.x > 1.0 || coords.y < 0.0 || coords.y > 1.0) {
        return vec4(0.0);
    }
    vec4 texColor = texture2D(tex, coords);
    #ifdef NO_ALPHA
    texColor = vec4(texColor.rgb, 1.0);
    #endif
    if (texColor.a > 0.0) {
        texColor.rgb /= texColor.a;
    }
    return texColor;
}

void main() {
    float prog = u_for_opening > 0.5 ? 1.0 - easeOutQuad(u_progress) : easeOutQuad(u_progress);

    // Сжатие текстуры по вертикали в тонкую полосу
    float scale = 1.0 / mix(1.0, SCALING, prog) - 1.0;
    vec2 coords = v_coords;
    coords.y    = coords.y * (scale + 1.0) - scale * 0.5;

    // Фазы схлопывания: по вертикали (TB), затем по горизонтали (LR), финальное затухание (FF)
    float tbProg = smoothstep(0.0, 1.0, clamp(prog / TB_TIME, 0.0, 1.0));
    float lrProg = smoothstep(0.0, 1.0, clamp((prog - LR_DELAY) / LR_TIME, 0.0, 1.0));
    float ffProg = smoothstep(0.0, 1.0, clamp((prog - 1.0 + FF_TIME) / FF_TIME, 0.0, 1.0));

    // Центрированные градиенты
    float tb = coords.y * 2.0;
    tb       = tb < 1.0 ? tb : 2.0 - tb;

    float lr = coords.x * 2.0;
    lr       = lr < 1.0 ? lr : 2.0 - lr;

    float tbMask = 1.0 - smoothstep(0.0, 1.0, clamp((tbProg - tb) / BLUR_WIDTH, 0.0, 1.0));
    float lrMask = 1.0 - smoothstep(0.0, 1.0, clamp((lrProg - lr) / BLUR_WIDTH, 0.0, 1.0));
    float ffMask = 1.0 - smoothstep(0.0, 1.0, ffProg);

    float mask = tbMask * lrMask * ffMask;

    vec4 oColor = getInputColor(coords);
    // Мягкий белый люминофорный импульс в момент коллапса луча
    oColor.rgb = mix(oColor.rgb, u_color.rgb * oColor.a, oColor.a * smoothstep(0.0, 1.0, prog));
    oColor.a *= mask;

    gl_FragColor = vec4(oColor.rgb * oColor.a * alpha, oColor.a * alpha);
}
