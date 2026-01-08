let video = null;
function setupMovie(movieId, timestamp) {
    video = document.getElementById("video");
    const videoSrc = `/movies/stream/${movieId}/index.m3u8`;

    if (Hls.isSupported()) {
        const hls = new Hls({
            startPosition: timestamp,
            currentTime: timestamp,
        });

        hls.loadSource(videoSrc);
        hls.attachMedia(video);

        console.log("HLS.js is supported and initialized.");
        initControls(timestamp);
        keyboardSupport();
        console.log("Controls initialized.");

        hls.on(Hls.Events.FRAG_CHANGED, function (event, data) {
            const currentTime = video.currentTime;
            saveProgress(movieId, currentTime);
            console.log("Fragment changed, current time:", currentTime);
        });
    } else if (video.canPlayType("application/vnd.apple.mpegurl")) {
        throw new Error("Native HLS playback not supported in this example.");
    }
    console.log("Video setup complete.");
}

function updateVolumeIcon(volume) {
    const iconMuted = document.getElementById("icon-muted");
    const iconMed = document.getElementById("icon-vol-med");
    iconMuted.classList.add("hidden");
    iconMed.classList.add("hidden");

    if (Number(volume) === 0) {
        iconMuted.classList.remove("hidden");
    } else {
        iconMed.classList.remove("hidden");
    }
}

function initControls(timestamp) {
    const container = document.getElementById("video-container");
    // const controls = document.getElementById("controls");
    // const overlay = document.getElementById("video-overlay");
    // const progressBar = document.getElementById("progress-bar");
    // const volumeContainer = document.getElementById("volume-container");
    const playPauseBtn = document.getElementById("play-pause-btn");
    const playLargeBtn = document.getElementById("play-large-btn");
    const progressContainer = document.getElementById("progress-container");
    const progressFilled = document.getElementById("progress-filled");
    const progressHandle = document.getElementById("progress-handle");
    const currentTime = document.getElementById("current-time");
    const duration = document.getElementById("duration");
    const volumeSilder = document.getElementById("volume-controls");
    const muteButton = document.getElementById("mute-btn");

    let isSeeking = false;

    function togglePlay() {
        if (video.paused) {
            video.play();
        } else {
            video.pause();
        }
        container.classList.toggle("playing");
    }

    video.addEventListener("timeupdate", () => {
        if (!isSeeking) {
            updateProgressBar();
        }
    });

    function updateProgressBar() {
        const time = video.currentTime;

        const progress = Math.round((time / video.duration) * 100);
        progressFilled.style.width = `${progress}%`;
        progressHandle.style.left = `${progress}%`;
        currentTime.textContent = formatTime(time);
    }

    function seek(e) {
        e.stopPropagation();
        const rect = progressContainer.getBoundingClientRect();
        const percent = Math.max(
            0,
            Math.min(1, (e.clientX - rect.left) / rect.width),
        );
        const time = percent * video.duration;

        progressFilled.style.width = `${percent * 100}%`;
        progressHandle.style.left = `${percent * 100}%`;
        video.currentTime = time;
        currentTime.textContent = formatTime(video.currentTime);
        console.log("seeking to", time);
        console.log("percent", percent);

        return percent;
    }

    console.log("Attaching click event listener to progressContainer");

    progressContainer.addEventListener("click", seek);

    progressContainer.addEventListener("mousedown", (e) => {
        isSeeking = true;
        const percent = seek(e);

        const onMove = (e) => seek(e);

        const onUp = (e) => {
            const finalPercent = seek(e);
            video.currentTime = finalPercent * video.duration;
            isSeeking = false;
            document.removeEventListener("mousemove", onMove);
            document.removeEventListener("mouseup", onUp);
        };

        document.addEventListener("mousemove", onMove);
        document.addEventListener("mouseup", onUp);
    });

    let saved_volume = 1;
    volumeSilder.addEventListener("input", (e) => {
        e.stopPropagation();
        volumeSilder.value = e.target.value;
        video.volume = e.target.value;
        saved_volume = video.volume;
        updateVolumeIcon(saved_volume);
    });
    volumeSilder.addEventListener("change", (e) => {
        e.stopPropagation();
        console.log("Volume changed to:", e.target.value);
        volumeSilder.value = e.target.value;
        video.volume = e.target.value;
        saved_volume = video.volume;
        updateVolumeIcon(saved_volume);
    });

    muteButton.addEventListener("click", (e) => {
        e.stopPropagation();
        if (video.volume === 0) {
            video.volume = saved_volume;
            volumeSilder.value = saved_volume;
            updateVolumeIcon(saved_volume);
        } else {
            saved_volume = video.volume;
            video.volume = 0;
            volumeSilder.value = 0;
            updateVolumeIcon(0);
        }
    });

    volumeSilder.addEventListener(
        "wheel",
        (e) => {
            e.preventDefault();
            e.stopPropagation();
            const delta = e.deltaY > 0 ? -0.05 : 0.05;
            const newVol = Math.max(
                0,
                Math.min(1, (video ? video.volume : saved_volume) + delta),
            );
            if (video) video.volume = newVol;
            volumeSilder.value = newVol;
            saved_volume = newVol;
            updateVolumeIcon(newVol);
        },
        { passive: false },
    );

    volumeSilder.addEventListener("click", (e) => {
        e.stopPropagation();
    });

    video.addEventListener("loadedmetadata", () => {
        duration.textContent = formatTime(video.duration);
        updateVolumeIcon(video.volume);

        console.log("timestamp at init");
        console.log(timestamp);

        const progress = Math.round((timestamp / video.duration) * 100);
        console.log(progress);
        progressFilled.style.width = `${progress}%`;
        progressHandle.style.left = `${progress}%`;
        currentTime.textContent = formatTime(timestamp);

        video.currentTime = timestamp;
    });

    container.addEventListener("click", (e) => {
        console.log("pause:", video.paused);
        if (video.paused) {
            video.play();
        } else {
            video.pause();
        }
        container.classList.toggle("playing");
    });

    playPauseBtn.addEventListener("click", (e) => {
        togglePlay;
    });
    playLargeBtn.addEventListener("click", (e) => {
        togglePlay;
    });
}

function controlVolume(volume) {
    if (video) {
        volume = volume / 100;
        video.volume = volume;
    }
}

function formatTime(seconds) {
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);

    if (h > 0) {
        return `${h}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
    }
    return `${m}:${s.toString().padStart(2, "0")}`;
}

function controlPlaybackSpeed(speed) {
    if (video) {
        video.playbackRate = speed;
    }
}

function toggleFullScreen() {
    const container = document.getElementById("video-container");
    if (!document.fullscreenElement) {
        container.requestFullscreen().catch((err) => {
            console.error(
                `Error attempting to enable full-screen mode: ${err.message} (${err.name})`,
            );
        });
    } else {
        document.exitFullscreen();
    }
}

function saveProgress(movieId, timestampInt) {
    apiFetch(`/movies/progress/${movieId}`, {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ timestamp: Math.floor(timestampInt) }),
    }).catch(console.error);
}

function keyboardSupport() {
    document.addEventListener("keydown", (e) => {
        if (e.target.tagName.toLowerCase() === "input") return;
        switch (e.key) {
            case "ArrowLeft":
                skipMovie(-5);
                e.preventDefault();
                break;
            case "ArrowRight":
                skipMovie(5);
                e.preventDefault();
                break;
            case "ArrowUp":
                adjustVolume(0.05);
                e.preventDefault();
                break;
            case "ArrowDown":
                adjustVolume(-0.05);
                e.preventDefault();
                break;
            case " ":
                if (video) {
                    if (video.paused) {
                        video.play();
                    } else {
                        video.pause();
                    }
                }
                e.preventDefault();
                break;
            default:
                break;
        }
    });
}

function skipMovie(seconds) {
    if (video) {
        video.currentTime = Math.max(
            -5,
            Math.min(video.duration, video.currentTime + seconds),
        );
    }
}

function adjustVolume(value) {
    if (video) {
        const newVolume = Math.max(0, Math.min(1, video.volume + value));
        video.volume = newVolume;
        const volumeSilder = document.getElementById("volume-controls");
        volumeSilder.value = newVolume;
        updateVolumeIcon(newVolume);
    }
}
