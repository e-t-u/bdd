/**
 * bdd - Master Documentation PDF Renderer with Callout Alert Styling & Base64 Image Embedding
 * Engine: Headless Chromium (Puppeteer) + KaTeX (Vector Math) + Mermaid.js (DOM Diagrams)
 */

const fs = require('fs');
const path = require('path');

// Target directory and input file from CLI args
const inputFile = process.argv[2];
const outputPdf = process.argv[3];
const assetsDir = process.argv[4] || path.join(__dirname, 'assets');

if (!inputFile || !outputPdf) {
    console.error("Usage: node render_pdf.js <input.md> <output.pdf> [assetsDir]");
    process.exit(1);
}

async function renderPDF() {
    const puppeteer = require('puppeteer');
    const { marked } = require('marked');

    // Read Markdown input
    let mdContent = fs.readFileSync(inputFile, 'utf8');

    // =========================================================================
    // BASE64 IMAGE EMBEDDING ENGINE
    // Converts relative/absolute image links (assets/logo/...) into base64 Data URIs
    // =========================================================================
    const rootDir = path.resolve(__dirname, '..');
    mdContent = mdContent.replace(/!\[(.*?)\]\(([^)]+)\)/g, (match, alt, imgPath) => {
        if (imgPath.startsWith('data:') || imgPath.startsWith('http://') || imgPath.startsWith('https://')) {
            return match;
        }
        try {
            let cleanPath = imgPath.replace(/^file:\/\/\//, '');
            if (!path.isAbsolute(cleanPath)) {
                const relToInput = path.resolve(path.dirname(inputFile), cleanPath);
                const relToRoot = path.resolve(rootDir, cleanPath);
                if (fs.existsSync(relToInput)) {
                    cleanPath = relToInput;
                } else if (fs.existsSync(relToRoot)) {
                    cleanPath = relToRoot;
                }
            }
            if (fs.existsSync(cleanPath)) {
                const ext = path.extname(cleanPath).toLowerCase();
                const mimeType = ext === '.svg' ? 'image/svg+xml' : (ext === '.jpg' || ext === '.jpeg' ? 'image/jpeg' : 'image/png');
                const base64Data = fs.readFileSync(cleanPath).toString('base64');
                return `![${alt}](data:${mimeType};base64,${base64Data})`;
            }
        } catch (e) {
            console.warn(`[WARN] Could not base64 embed image: ${imgPath}`, e);
        }
        return match;
    });



    // Replace standalone tildes (~17.4%) with &approx; to prevent GFM strikethrough (<del>)
    mdContent = mdContent.replace(/~~/g, '___DOUBLE_TILDE___');
    mdContent = mdContent.replace(/~/g, '&approx;');
    mdContent = mdContent.replace(/___DOUBLE_TILDE___/g, '~~');

    // Detect landscape & slide decks
    const isLandscape = /size:\s*landscape|orientation:\s*landscape|16in\s*9in|@page\s*\{\s*size:\s*landscape|marp:\s*true|slides:\s*true/i.test(mdContent);

    // Pagebreak & Slide break transformer
    mdContent = mdContent.replace(/<!--\s*(?:page-?break|slide)\s*-->/gi, '\n<div class="page-break"></div>\n');

    // If document is landscape or contains Marp / Slide Deck markers, convert slide separator `---` to page breaks
    if (isLandscape || /marp:\s*true/i.test(mdContent) || /Slide\s+\d+:/i.test(mdContent)) {
        mdContent = mdContent.replace(/\n---\n/g, '\n<div class="page-break"></div>\n');
    }


    // =========================================================================
    // GITHUB ALERT / CALLOUT BOX TRANSFORMER ([!NOTE], [!TIP], [!IMPORTANT], etc.)
    // Converts GFM alert blocks into styled executive callout boxes with icons
    // =========================================================================
    mdContent = mdContent.replace(/^>\s*\[\!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\][ \t]*(.*(?:\n>.*)*)/gm, (match, type, content) => {
        const cleanBody = content.replace(/^>\s?/gm, '').trim();
        const typeUpper = type.toUpperCase();
        let icon = 'ℹ️';
        if (typeUpper === 'TIP') icon = '💡';
        if (typeUpper === 'IMPORTANT') icon = '📌';
        if (typeUpper === 'WARNING') icon = '⚠️';
        if (typeUpper === 'CAUTION') icon = '🚨';
        
        return `<div class="callout callout-${typeUpper.toLowerCase()}"><div class="callout-title"><span class="callout-icon">${icon}</span> ${typeUpper}</div><div class="callout-body">\n\n${cleanBody}\n\n</div></div>\n\n`;
    });

    // =========================================================================
    // BULLETPROOF LATEX MATH PROTECTION & SERVER-SIDE KATEX ENGINE
    // =========================================================================
    const katex = require('katex');

    // 0. Sanitize ASCII control character corruptions & convert un-delimited \rightarrow to Unicode →:
    mdContent = mdContent
        .replace(/\\+\s*rightarrow/gi, '→')
        .replace(/\$\s*\\+rightarrow\s*\$/gi, '→')
        .replace(/\x08egin/g, '\\begin')
        .replace(/\x08/g, '\\b')
        .replace(/\x09ext/g, '\\text')
        .replace(/\x09imes/g, '\\times')
        .replace(/\x0crac/g, '\\frac')
        .replace(/\x0c/g, '\\f')
        .replace(/\x0dight/g, '\\right')
        .replace(/\x0delt/g, '\\delta');


    // Wrap any standalone \begin{aligned} ... \end{aligned} blocks in $$...$$
    mdContent = mdContent.replace(/(^|\n)(\\begin\{(?:aligned|equation|matrix)\}[\s\S]*?\\end\{(?:aligned|equation|matrix)\})/g, '$1$$\n$2\n$$');

    const mathBlocks = [];

    function renderMathToken(mathStr, displayMode) {
        let cleanMath = mathStr.trim();
        if (cleanMath.startsWith('$$') && cleanMath.endsWith('$$')) {
            cleanMath = cleanMath.slice(2, -2).trim();
        } else if (cleanMath.startsWith('\\[') && cleanMath.endsWith('\\]')) {
            cleanMath = cleanMath.slice(2, -2).trim();
        } else if (cleanMath.startsWith('\\(') && cleanMath.endsWith('\\)')) {
            cleanMath = cleanMath.slice(2, -2).trim();
        } else if (cleanMath.startsWith('$') && cleanMath.endsWith('$')) {
            cleanMath = cleanMath.slice(1, -1).trim();
        }

        cleanMath = cleanMath
            .replace(/\x08egin/g, '\\begin')
            .replace(/\x08/g, '\\b')
            .replace(/\x09ext/g, '\\text')
            .replace(/\x09imes/g, '\\times')
            .replace(/\x0crac/g, '\\frac')
            .replace(/\x0c/g, '\\f')
            .replace(/\x0dight/g, '\\right')
            .replace(/\x0delt/g, '\\delta')
            .replace(/\\{2,}([%$_{}#&])/g, '\\$1')
            .replace(/\\{2,}([a-zA-Z]+)/g, '\\$1')
            .replace(/\\{4,}/g, '\\\\');

        // Sanitize & and € inside \text{...} to prevent KaTeX alignment parse errors
        cleanMath = cleanMath.replace(/\\text\{([^}]+)\}/g, (m, textContent) => {
            return '\\text{' + textContent.replace(/&/g, 'and').replace(/€/g, 'EUR ') + '}';
        });
        cleanMath = cleanMath.replace(/€/g, '\\text{EUR }');

        try {
            const res = katex.renderToString(cleanMath, {
                displayMode: displayMode,
                throwOnError: false
            });
            if (res.includes('katex-error')) {
                console.warn(`[WARN] KaTeX parse warning for math token: "${cleanMath.slice(0, 60)}..."`);
            }
            return res;
        } catch (e) {
            console.warn(`[WARN] KaTeX render error: ${e.message}`);
            return mathStr;
        }
    }


    // 0. Protect code blocks from KaTeX math extraction
    const codeBlocks = [];
    mdContent = mdContent.replace(/(```[\s\S]*?```|`[^`\n]+`)/g, (match) => {
        const token = `CODEBLOCKPROTECTED${codeBlocks.length}TOKEN`;
        codeBlocks.push(match);
        return token;
    });

    // 1. Extract & Server-Render Block Math ($$...$$ or \[...\])
    mdContent = mdContent.replace(/(\$\$[\s\S]*?\$\$|\\\[[\s\S]*?\\\])/g, (match) => {
        const token = `KATEXMATHBLOCK${mathBlocks.length}TOKEN`;
        const rendered = renderMathToken(match, true);
        mathBlocks.push(rendered);
        return token;
    });

    // 2. Extract & Server-Render Inline Math (\(... \) or $...$)
    mdContent = mdContent.replace(/(\\\([\s\S]*?\\\)|(?:\$[^$\n]+\$))/g, (match) => {
        if (/\\|\_|\^|\=|\{|\}/.test(match)) {
            const token = `KATEXMATHBLOCK${mathBlocks.length}TOKEN`;
            const rendered = renderMathToken(match, false);
            mathBlocks.push(rendered);
            return token;
        }
        return match;
    });

    // Restore protected code blocks
    codeBlocks.forEach((code, index) => {
        mdContent = mdContent.replace(`CODEBLOCKPROTECTED${index}TOKEN`, () => code);
    });

    // Custom marked renderer to convert ```mermaid blocks into <div class="mermaid">
    const renderer = new marked.Renderer();
    const originalCode = renderer.code.bind(renderer);

    renderer.code = (code, language, isEscaped) => {
        if (language === 'mermaid') {
            return `<div class="mermaid">\n${code}\n</div>`;
        }
        return originalCode(code, language, isEscaped);
    };

    marked.use({ renderer });

    // Parse Markdown into HTML
    let htmlBody = marked.parse(mdContent);

    // 3. Restore pre-rendered KaTeX HTML into parsed markdown
    mathBlocks.forEach((renderedHtml, index) => {
        const token = `KATEXMATHBLOCK${index}TOKEN`;
        const pRegExp = new RegExp(`<p>\\s*${token}\\s*<\\/p>`, 'g');
        if (renderedHtml.includes('katex-display')) {
            htmlBody = htmlBody.replace(pRegExp, `<div class="katex-display-container">${renderedHtml}</div>`);
        }
        htmlBody = htmlBody.split(token).join(renderedHtml);
    });

    // If landscape or slide deck, wrap each page chunk into beautiful slide containers
    if (isLandscape) {
        let slideChunks = htmlBody.split(/<div class="page-break"><\/div>/i).map(c => c.trim()).filter(c => c.length > 0);
        
        // If the last chunk is just a tiny footer note (no h1, h2, h3), merge it into the previous slide
        if (slideChunks.length > 1) {
            const lastChunk = slideChunks[slideChunks.length - 1];
            if (!/<h[1-3]/i.test(lastChunk) && lastChunk.length < 300) {
                slideChunks[slideChunks.length - 2] += `\n<div class="slide-footer-note">${lastChunk}</div>`;
                slideChunks.pop();
            }
        }

        if (slideChunks.length > 0) {
            htmlBody = slideChunks.map((chunk, idx) => {
                const clean = chunk.trim();
                // Check if title / cover slide (first slide and has h1 or no h2)
                const isCover = (idx === 0) && (clean.includes('<h1') || !clean.includes('<h2'));
                const slideClass = isCover ? 'slide-deck-slide slide-cover-page' : 'slide-deck-slide slide-content-page';
                return `<div class="${slideClass}">\n${clean}\n</div>`;
            }).join('\n<div class="page-break"></div>\n');
        }
    }
        
    // Extract document title dynamically from markdown (# Header, frontmatter, or filename)
    let docTitle = path.basename(inputFile, path.extname(inputFile));
    const titleMatch = mdContent.match(/^#\s+(.+)$/m);
    if (titleMatch && titleMatch[1]) {
        docTitle = titleMatch[1].replace(/[*_`]/g, '').trim();
    }
    const escapedTitle = docTitle.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

    // Full Executive HTML Template with KaTeX & Mermaid.js
    const fullHtml = `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>${escapedTitle}</title>
    <!-- Google Fonts for High-Fidelity Typography -->
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;500;600;700&display=swap" rel="stylesheet">
    <!-- KaTeX CSS for Vector LaTeX Math Rendering -->
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/katex.min.css">
    <script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/katex.min.js"></script>
    <script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/contrib/auto-render.min.js"
        onload="renderMathInElement(document.body, {
            delimiters: [
                {left: '$$', right: '$$', display: true},
                {left: '\\\\[', right: '\\\\]', display: true},
                {left: '\\\\(', right: '\\\\)', display: false},
                {left: '$', right: '$', display: false}
            ],
            throwOnError : false
        });"></script>
    <!-- Mermaid.js for Native Vector Diagram Rendering -->
    <script src="https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.min.js"></script>
    <style>
        @page {
            size: ${isLandscape ? 'A4 landscape' : 'A4 portrait'};
            margin: ${isLandscape ? '15mm 15mm 15mm 15mm' : '20mm 15mm 20mm 15mm'};
        }
        *, *::before, *::after {
            box-sizing: border-box;
        }
        body {
            font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
            font-size: 10pt;
            line-height: 1.65;
            color: #1E293B;
            margin: 0;
            padding: 0;
            background: #FFFFFF;
            -webkit-font-smoothing: antialiased;
            -moz-osx-font-smoothing: grayscale;
            text-rendering: geometricPrecision;
            font-feature-settings: "kern" 1, "liga" 1, "calt" 1;
        }
        h1, h2, h3, h4 {
            color: #0F2C59;
            font-weight: 700;
            page-break-after: avoid;
        }
        h1 {
            font-size: 18pt;
            border-bottom: 2.5px solid #0F2C59;
            padding-bottom: 5px;
            margin-top: 18pt;
            letter-spacing: -0.5px;
        }
        h2 {
            font-size: 14pt;
            border-bottom: 1.5px solid #D4AF37;
            padding-bottom: 3px;
            margin-top: 14pt;
        }
        h3 {
            font-size: 11.5pt;
            margin-top: 12pt;
            color: #0F2C59;
        }
        p, ul, ol {
            margin-bottom: 8pt;
        }

        hr {
            border: none;
            border-top: 1.5px solid #CBD5E1;
            margin: 16pt 0;
            page-break-after: auto;
        }

        .page-break, hr.page-break {
            page-break-after: always !important;
            break-after: always !important;
            height: 0;
            margin: 0;
            padding: 0;
            border: none;
        }

        /* 16:9 WIDESCREEN PITCH DECK SLIDE STYLING */
        .slide-deck {
            font-family: 'Inter', 'Outfit', sans-serif;
        }
        .slide-deck-slide {
            width: 100%;
            min-height: 160mm;
            max-height: 172mm;
            box-sizing: border-box;
            display: flex;
            flex-direction: column;
            justify-content: flex-start;
            padding: 20pt 26pt;
            border-radius: 12px;
            position: relative;
            page-break-inside: avoid !important;
            break-inside: avoid !important;
            overflow: hidden;
        }
        .slide-cover-page {
            background: linear-gradient(135deg, #0B192C 0%, #1E3E62 55%, #000000 100%);
            color: #FFFFFF;
            justify-content: center;
            align-items: center;
            text-align: center;
            border: 1px solid #1E293B;
            box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
        }
        .slide-cover-page h1 {
            font-size: 28pt;
            font-weight: 800;
            color: #FFFFFF;
            margin: 0 0 12pt 0;
            padding: 0;
            border-bottom: none;
            letter-spacing: -0.8px;
            line-height: 1.2;
            background: linear-gradient(90deg, #FFFFFF, #93C5FD, #E0F2FE);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
        }
        .slide-cover-page h2, .slide-cover-page h3, .slide-cover-page p {
            font-size: 12.5pt;
            color: #94A3B8;
            margin: 4pt 0 8pt 0;
            font-weight: 400;
            line-height: 1.5;
            max-width: 85%;
        }
        .slide-cover-page ul {
            list-style: none;
            padding: 0;
            margin: 10pt auto;
            display: flex;
            flex-direction: column;
            gap: 7pt;
            text-align: left;
            width: 100%;
            max-width: 90%;
        }
        .slide-cover-page ul li {
            background: rgba(255, 255, 255, 0.08);
            border: 1px solid rgba(255, 255, 255, 0.15);
            border-left: 4px solid #38BDF8;
            border-radius: 8px;
            padding: 8pt 12pt;
            font-size: 10pt;
            color: #E2E8F0;
            line-height: 1.45;
        }
        .slide-cover-page ul li strong {
            color: #38BDF8;
        }
        .slide-cover-page .callout {
            background: rgba(15, 23, 42, 0.85);
            border: 1px solid rgba(56, 189, 248, 0.4);
            border-left: 4px solid #38BDF8;
            color: #E2E8F0;
            text-align: left;
            max-width: 90%;
            margin: 8pt auto 0 auto;
        }
        .slide-cover-page .callout-title {
            color: #38BDF8;
        }
        .slide-cover-page .callout-body {
            color: #CBD5E1;
        }
        .slide-content-page {
            background: #FFFFFF;
            border: 1px solid #E2E8F0;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.04);
        }
        .slide-content-page h2 {
            font-size: 18pt;
            font-weight: 700;
            color: #0F2C59;
            margin: 0 0 12pt 0;
            padding-bottom: 6pt;
            border-bottom: 2.5px solid #0284C7;
            letter-spacing: -0.4px;
        }
        .slide-content-page h3 {
            font-size: 12pt;
            color: #0284C7;
            margin: 8pt 0 4pt 0;
            font-weight: 600;
        }
        .slide-content-page ul {
            list-style: none;
            padding: 0;
            margin: 0 0 10pt 0;
            display: grid;
            grid-template-columns: 1fr;
            gap: 8pt;
        }
        .slide-content-page ul li {
            background: #F8FAFC;
            border: 1px solid #E2E8F0;
            border-left: 4.5px solid #0284C7;
            border-radius: 8px;
            padding: 9pt 13pt;
            font-size: 10pt;
            line-height: 1.45;
            color: #334155;
            box-shadow: 0 1px 3px rgba(0, 0, 0, 0.02);
        }
        .slide-content-page ul li strong {
            color: #0F2C59;
            font-weight: 700;
        }
        .slide-card {
            background-color: #F8FAFC;
            border: 1px solid #CBD5E1;
            border-radius: 8px;
            padding: 12pt 16pt;
            margin-bottom: 12pt;
            page-break-inside: avoid;
        }
        .slide-card-dark {
            background-color: #0F2C59;
            color: #FFFFFF;
            border-radius: 8px;
            padding: 12pt 16pt;
            margin-bottom: 12pt;
            page-break-inside: avoid;
        }

        /* NOTEBOOKLM-STYLE VISUAL SLIDE BACKGROUND CARDS */
        .slide-card-hero {
            background: linear-gradient(135deg, rgba(15, 44, 89, 0.85), rgba(15, 23, 42, 0.92)), url('assets/backgrounds/hero_nordic_asean_bg.jpg') no-repeat center center / cover;
            color: #FFFFFF;
            border-radius: 12px;
            padding: 16pt 20pt;
            margin-bottom: 12pt;
            border: 1px solid rgba(255, 255, 255, 0.2);
            page-break-inside: avoid;
        }
        .slide-card-regulatory {
            background: linear-gradient(135deg, rgba(15, 44, 89, 0.85), rgba(15, 23, 42, 0.92)), url('assets/backgrounds/regulatory_compliance_bg.jpg') no-repeat center center / cover;
            color: #FFFFFF;
            border-radius: 12px;
            padding: 16pt 20pt;
            margin-bottom: 12pt;
            border: 1px solid rgba(255, 255, 255, 0.2);
            page-break-inside: avoid;
        }
        .slide-card-finance {
            background: linear-gradient(135deg, rgba(15, 44, 89, 0.85), rgba(15, 23, 42, 0.92)), url('assets/backgrounds/financial_growth_bg.jpg') no-repeat center center / cover;
            color: #FFFFFF;
            border-radius: 12px;
            padding: 16pt 20pt;
            margin-bottom: 12pt;
            border: 1px solid rgba(255, 255, 255, 0.2);
            page-break-inside: avoid;
        }
        .slide-card-executive {
            background: linear-gradient(135deg, rgba(15, 44, 89, 0.85), rgba(15, 23, 42, 0.92)), url('assets/backgrounds/executive_pitch_bg.jpg') no-repeat center center / cover;
            color: #FFFFFF;
            border-radius: 12px;
            padding: 16pt 20pt;
            margin-bottom: 12pt;
            border: 1px solid rgba(255, 255, 255, 0.2);
            page-break-inside: avoid;
        }


        /* LOGO & IMAGE CONTAINMENT - EXECUTIVE STYLING */
        img {
            max-width: 100%;
            max-height: 120px;
            height: auto;
            display: block;
            margin: 10pt auto 16pt auto;
            object-fit: contain;
            page-break-inside: avoid;
        }

        /* CALLOUT / ALERT BOX STYLING ([!NOTE], [!TIP], [!IMPORTANT], etc.) */
        .callout {
            margin: 12pt 0;
            padding: 10pt 14pt;
            border-left: 4px solid #0052CC;
            background-color: #F8FAFC;
            border-radius: 0 6px 6px 0;
            page-break-inside: avoid;
        }
        .callout-title {
            font-weight: 700;
            font-size: 9.5pt;
            text-transform: uppercase;
            letter-spacing: 0.5px;
            margin-bottom: 4pt;
            display: flex;
            align-items: center;
            gap: 6px;
        }
        .callout-icon {
            font-size: 11pt;
        }
        .callout-body {
            font-size: 9.5pt;
            line-height: 1.5;
            color: #334155;
        }
        .callout-body p:last-child {
            margin-bottom: 0;
        }

        .callout-note { border-left-color: #0052CC; background-color: #EFF6FF; }
        .callout-note .callout-title { color: #1E40AF; }
        
        .callout-tip { border-left-color: #10B981; background-color: #ECFDF5; }
        .callout-tip .callout-title { color: #065F46; }
        
        .callout-important { border-left-color: #8B5CF6; background-color: #F5F3FF; }
        .callout-important .callout-title { color: #5B21B6; }

        .callout-warning { border-left-color: #F59E0B; background-color: #FFFBEB; }
        .callout-warning .callout-title { color: #92400E; }

        .callout-caution { border-left-color: #EF4444; background-color: #FEF2F2; }
        .callout-caution .callout-title { color: #991B1B; }

        /* MERMAID DIAGRAM VECTOR SVG DOM STYLING */
        .mermaid, div.mermaid {
            width: 100% !important;
            max-width: 100% !important;
            min-width: 100% !important;
            height: auto !important;
            display: block !important;
            margin: 18pt auto !important;
            text-align: center;
            page-break-inside: avoid;
        }
        .mermaid svg {
            width: 100% !important;
            max-width: 100% !important;
            height: auto !important;
            display: block !important;
            margin: 0 auto !important;
        }

        /* KATEX VECTOR MATHEMATICS STYLING */
        .katex-display {
            display: block;
            margin: 12pt 0;
            text-align: center;
            overflow-x: auto;
            overflow-y: hidden;
            max-width: 100%;
        }
        .katex {
            font-size: 1.05em;
            color: #0F2C59;
        }

        /* EXECUTIVE TABLE STYLING */
        table {
            width: 100%;
            border-collapse: separate;
            border-spacing: 0;
            margin: 14pt 0;
            font-size: 9pt;
            border: 1px solid #CBD5E1;
            border-radius: 5px;
            overflow: hidden;
            page-break-inside: avoid;
        }
        th {
            background-color: #0F2C59 !important;
            color: #FFFFFF !important;
            font-weight: 700;
            font-size: 9.5pt;
            text-transform: uppercase;
            letter-spacing: 0.5px;
            padding: 8pt 10pt;
            border-bottom: 3px solid #D4AF37 !important;
            border-right: 1px solid #1E3A8A;
        }
        th:last-child {
            border-right: none;
        }
        td {
            padding: 7pt 10pt;
            border-bottom: 1px solid #E2E8F0;
            border-right: 1px solid #F1F5F9;
            color: #1E293B;
        }
        td:last-child {
            border-right: none;
        }
        tr:last-child td {
            border-bottom: none;
        }
        tr:nth-child(even) td {
            background-color: #F8FAFC !important;
        }

        blockquote {
            border-left: 4px solid #D4AF37;
            background-color: #F8FAFC;
            margin: 10pt 0;
            padding: 8pt 12pt;
            font-style: italic;
            border-radius: 0 4px 4px 0;
        }

        code {
            background-color: #F1F5F9 !important;
            color: #0F2C59 !important;
            padding: 0.15em 0.4em;
            border-radius: 3pt;
            font-family: 'JetBrains Mono', 'Roboto Mono', 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
            font-size: 8.5pt;
            line-height: 1.6;
            vertical-align: baseline;
        }
        pre {
            background-color: #F8FAFC !important;
            color: #0F2C59 !important;
            border: 1px solid #CBD5E1 !important;
            padding: 10pt 12pt;
            border-radius: 6pt;
            overflow: visible !important;
            white-space: pre-wrap !important;
            word-wrap: break-word !important;
            page-break-inside: avoid;
            font-family: 'JetBrains Mono', 'Roboto Mono', 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
            font-size: 8.5pt;
            line-height: 1.65;
            margin: 10pt 0 14pt 0;
        }
        pre code {
            background-color: transparent !important;
            padding: 0 !important;
            border-radius: 0;
            font-size: inherit;
            line-height: inherit;
            color: inherit;
            display: block;
            overflow: visible !important;
        }
    </style>
</head>
<body>
    ${htmlBody}
    <script>
        mermaid.initialize({
            startOnLoad: false,
            theme: 'neutral',
            securityLevel: 'loose',
            flowchart: { useMaxWidth: true, htmlLabels: true },
            gantt: { useMaxWidth: true },
            sequence: { useMaxWidth: true }
        });

        // Convert any pre/code blocks containing mermaid into <div class="mermaid">
        document.querySelectorAll('pre code.language-mermaid, pre.language-mermaid code, pre.mermaid, code.language-mermaid').forEach(el => {
            const parent = el.closest('pre') || el;
            const div = document.createElement('div');
            div.className = 'mermaid';
            div.textContent = el.textContent;
            parent.replaceWith(div);
        });

        // Trigger Mermaid rendering
        if (window.mermaid) {
            window.mermaid.run();
        }
    </script>
</body>
</html>`;

    // Launch Headless Chromium via Puppeteer with unhinted vector font rendering
    const puppeteerOptions = {
        args: [
            '--no-sandbox',
            '--disable-setuid-sandbox',
            '--disable-dev-shm-usage',
            '--allow-file-access-from-files',
            '--font-render-hinting=none',
            '--enable-font-antialiasing',
            '--force-color-profile=srgb'
        ]
    };

    if (fs.existsSync('/usr/bin/chromium-browser')) {
        puppeteerOptions.executablePath = '/usr/bin/chromium-browser';
    } else if (fs.existsSync('/usr/bin/chromium')) {
        puppeteerOptions.executablePath = '/usr/bin/chromium';
    }

    const browser = await puppeteer.launch(puppeteerOptions);
    const page = await browser.newPage();
    await page.setContent(fullHtml, { waitUntil: 'networkidle0' });

    // Wait for web fonts and trigger KaTeX and Mermaid DOM rendering
    await page.evaluate(async () => {
        if (document.fonts && document.fonts.ready) {
            await document.fonts.ready;
        }
        if (window.renderMathInElement) {
            window.renderMathInElement(document.body, {
                delimiters: [
                    {left: '$$', right: '$$', display: true},
                    {left: '\\[', right: '\\]', display: true},
                    {left: '\\(', right: '\\)', display: false},
                    {left: '$', right: '$', display: false}
                ],
                throwOnError: false
            });
        }
        if (window.mermaid) {
            await window.mermaid.run();
        }
    });
    await new Promise(r => setTimeout(r, 1500));

    // Export PDF via Chromium Engine
    await page.pdf({
        path: outputPdf,
        format: 'A4',
        landscape: isLandscape,
        margin: isLandscape
            ? { top: '15mm', right: '15mm', bottom: '15mm', left: '15mm' }
            : { top: '20mm', right: '15mm', bottom: '20mm', left: '15mm' },
        printBackground: true,
        displayHeaderFooter: true,
        headerTemplate: '<div></div>',
        footerTemplate: '<div style="font-family: sans-serif; font-size: 8pt; color: #64748B; width: 100%; text-align: right; padding-right: 15mm;">Page <span class="pageNumber"></span> of <span class="totalPages"></span></div>'
    });

    await browser.close();
    console.log(`[PUPPETEER PDF] Successfully generated ${outputPdf}`);
}

renderPDF().catch(err => {
    console.error("PDF Generation Error:", err);
    process.exit(1);
});
