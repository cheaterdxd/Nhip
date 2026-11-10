
requestAnimationFrame(()=>requestAnimationFrame(async()=>{await window.__TAURI_INTERNALS__.invoke('finish_benchmark',{measurements:{minimal:true,navigation_to_ready_ms:performance.now(),user_agent:navigator.userAgent}})}));
