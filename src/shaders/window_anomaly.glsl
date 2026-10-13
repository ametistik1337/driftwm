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

// 2 out, 2 in hash (David Hoskins)
vec2 hash22(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

// 2D Simplex Noise (Inigo Quilez, MIT)
float simplex2D(vec2 p) {
    const float K1 = 0.366025404; // (sqrt(3)-1)/2
    const float K2 = 0.211324865; // (3-sqrt(3))/6

    vec2 i = floor(p + (p.x + p.y) * K1);
    vec2 a = p - i + (i.x + i.y) * K2;
    float m = step(a.y, a.x);
    vec2 o = vec2(m, 1.0 - m);
    vec2 b = a - o + K2;
    vec2 c = a - 1.0 + 2.0 * K2;
    vec3 h = max(0.5 - vec3(dot(a, a), dot(b, b), dot(c, c)), 0.0);
    vec3 n = h * h * h * h * vec3(
        dot(a, -1.0 + 2.0 * hash22(i + 0.0)),
        dot(b, -1.0 + 2.0 * hash22(i + o)),
        dot(c, -1.0 + 2.0 * hash22(i + 1.0))
    );
    return 0.5 + 0.5 * dot(n, vec3(70.0));
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
    float prog = u_for_opening > 0.5 ? 1.0 - u_progress : u_progress;

    vec2 center = vec2(0.5, 0.5);
    vec2 aspect = vec2(u_size.x / max(u_size.y, 1.0), 1.0);
    vec2 delta = (v_coords - center) * aspect;
    float dist = length(delta);
    vec2 dir = dist > 0.0001 ? normalize(delta / aspect) : vec2(0.0, 1.0);

    // Радиус распространения ударной волны гравитационного всплеска
    float maxR = 1.4 * max(aspect.x, 1.0);
    float waveRadius = pow(prog, 0.8) * maxR;

    // Профиль ударной волны всплеска аномалии (сжатие + разряжение)
    float dr = dist - waveRadius;
    float waveWidth = 0.16 * (1.0 + prog * 0.5);
    float waveNorm = dr / waveWidth;
    float shockFactor = exp(-waveNorm * waveNorm * 3.5) * sin(-waveNorm * 3.14159265 * 2.2);
    float shockAmp = (1.0 - prog * 0.7) * 0.14 * smoothstep(0.0, 0.07, prog);
    vec2 shockOffset = dir * shockFactor * shockAmp;

    // Внутри волны: гравитационное линзирование и вихрь сингулярности
    float inside = smoothstep(waveRadius, 0.0, dist);
    float pull = inside * (1.0 - dist / max(waveRadius, 0.001)) * pow(prog, 0.6) * 0.32;
    vec2 pullOffset = -dir * pull;

    // Вихревое закручивание пространства («Воронка»)
    float swirlAngle = inside * (1.0 - dist / max(waveRadius, 0.001)) * prog * 4.2;
    float s = sin(swirlAngle);
    float c = cos(swirlAngle);
    mat2 rot = mat2(c, -s, s, c);

    // Пространственное марево / тепловое искажение воздуха
    vec2 noiseCoord = v_coords * 7.0 + vec2(0.0, prog * 3.5);
    float n = simplex2D(noiseCoord);
    vec2 shimmerOffset = vec2(cos(n * 6.28318), sin(n * 6.28318)) * 0.025 * (1.0 - prog) * inside;

    // Итоговые искаженные координаты
    vec2 centered = v_coords - center;
    centered = rot * centered;
    vec2 distortedCoords = center + centered + shockOffset + pullOffset + shimmerOffset;

    // Спектральная дисперсия (хроматическое преломление на гребне волны)
    float dispersion = abs(shockFactor) * shockAmp * 0.45;
    vec4 colR = getInputColor(distortedCoords + dir * dispersion);
    vec4 colG = getInputColor(distortedCoords);
    vec4 colB = getInputColor(distortedCoords - dir * dispersion);

    vec3 col = vec3(colR.r, colG.g, colB.b);
    float sampleAlpha = colG.a;

    // Ионизация и холодное свечение гребня аномалии
    float ionization = clamp(shockFactor, 0.0, 1.0) * (1.0 - prog) * 0.38;
    col += vec3(0.65, 0.88, 1.0) * ionization;

    // Растворение окна в эпицентре аномалии
    float collapseAlpha = clamp(1.0 - prog * 1.3, 0.0, 1.0);
    float dissolveInside = smoothstep(0.0, waveRadius * 0.5, dist);
    float winFade = mix(collapseAlpha, 1.0 - prog, dissolveInside);

    float finalAlpha = sampleAlpha * winFade * alpha;

    if (distortedCoords.x < 0.0 || distortedCoords.x > 1.0 || distortedCoords.y < 0.0 || distortedCoords.y > 1.0) {
        finalAlpha = 0.0;
    }

    gl_FragColor = vec4(col * finalAlpha, finalAlpha);
}
