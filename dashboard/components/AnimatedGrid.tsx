'use client';

import { useEffect, useRef } from 'react';

interface Point {
  x: number;
  y: number;
  baseX: number;
  baseY: number;
  offsetZ: number;
}

// Simple Perlin-like noise function
function noise(x: number, y: number, time: number): number {
  const scale = 0.015;
  const timeScale = 0.0003;
  const nx = x * scale;
  const ny = y * scale;
  const nt = time * timeScale;

  // Combine multiple sine waves for organic-looking noise
  const noise1 = Math.sin(nx + nt) * Math.cos(ny + nt);
  const noise2 = Math.sin(nx * 2.3 + nt * 1.7) * Math.cos(ny * 1.8 + nt * 1.3);
  const noise3 = Math.sin(nx * 3.7 - nt * 0.9) * Math.cos(ny * 2.9 - nt * 1.1);

  return (noise1 + noise2 * 0.5 + noise3 * 0.25) / 1.75;
}

export default function AnimatedGrid() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const mouseRef = useRef({ x: -1000, y: -1000, lastMoveTime: 0 });
  const animationRef = useRef<number | undefined>(undefined);
  const timeRef = useRef(0);
  const pulseRef = useRef(0);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    // Disable on mobile devices
    if (window.innerWidth < 768) {
      return;
    }

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    let cols = 0;
    let rows = 0;
    let points: Point[] = [];

    // Set canvas size
    const resize = () => {
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;

      // Grid configuration (smaller by ~1/3)
      const gridSpacing = 35;
      cols = Math.ceil(canvas.width / gridSpacing) + 1;
      rows = Math.ceil(canvas.height / gridSpacing) + 1;
      points = [];

      // Create grid points with initial positions
      for (let i = 0; i < rows; i++) {
        for (let j = 0; j < cols; j++) {
          const baseX = j * gridSpacing;
          const baseY = i * gridSpacing;
          points.push({
            x: baseX,
            y: baseY,
            baseX: baseX,
            baseY: baseY,
            offsetZ: 0,
          });
        }
      }
    };
    resize();
    window.addEventListener('resize', resize);

    // Mouse tracking
    const handleMouseMove = (e: MouseEvent) => {
      mouseRef.current = { x: e.clientX, y: e.clientY, lastMoveTime: Date.now() };
    };
    window.addEventListener('mousemove', handleMouseMove);

    // Animation loop
    const animate = () => {
      timeRef.current += 1;

      // Semi-transparent background for fade effect
      ctx.fillStyle = 'rgba(255, 255, 255, 0.1)';
      ctx.fillRect(0, 0, canvas.width, canvas.height);

      // Continuous pulse effect (always active, not only when idle)
      pulseRef.current += 0.03;
      const pulseEffect = Math.sin(pulseRef.current) * 0.5 + 0.5; // 0 to 1
      const rotationEffect = Math.cos(pulseRef.current * 0.7); // For circular motion

      // Update points with wave motion and mouse influence
      const waveAmplitude = 15;
      const mouseInfluence = 150;
      const mouseStrength = 40; // More smooth, less responsive

      points.forEach((point) => {
        // Wave animation using noise
        const noiseValue = noise(point.baseX, point.baseY, timeRef.current);
        point.offsetZ = noiseValue * waveAmplitude;

        // Calculate distance to mouse
        const dx = mouseRef.current.x - point.baseX;
        const dy = mouseRef.current.y - point.baseY;
        const distance = Math.sqrt(dx * dx + dy * dy);

        // Mouse repulsion effect with continuous subtle pulse and rotation
        let mouseOffsetX = 0;
        let mouseOffsetY = 0;
        if (distance < mouseInfluence && distance > 0) {
          const baseForce = (1 - distance / mouseInfluence) * mouseStrength;

          // Subtle continuous pulse (2x more noticeable)
          const pulseBoost = distance < 100 ? pulseEffect * 3 : 0;

          // Circular motion component (clockwise/counterclockwise)
          const angle = Math.atan2(dy, dx);
          const tangentX = -Math.sin(angle); // Perpendicular to radial direction
          const tangentY = Math.cos(angle);
          const circularForce = distance < 100 ? rotationEffect * 2 : 0;

          const force = baseForce + pulseBoost;
          mouseOffsetX = -(dx / distance) * force + tangentX * circularForce;
          mouseOffsetY = -(dy / distance) * force + tangentY * circularForce;
        }

        // Apply offsets with smoothing (more viscous/sticky movement)
        const smoothFactor = 0.02; // Very low = very smooth/sticky, less reactive
        point.x = point.x * (1 - smoothFactor) + (point.baseX + mouseOffsetX) * smoothFactor;
        point.y = point.y * (1 - smoothFactor) + (point.baseY + mouseOffsetY + point.offsetZ) * smoothFactor;
      });

      // Draw horizontal lines with smooth gradient
      for (let i = 0; i < rows; i++) {
        ctx.beginPath();
        for (let j = 0; j < cols; j++) {
          const point = points[i * cols + j];

          if (j === 0) {
            ctx.moveTo(point.x, point.y);
          } else {
            ctx.lineTo(point.x, point.y);
          }
        }

        // Calculate color based on wave height and mouse proximity
        const midPoint = points[i * cols + Math.floor(cols / 2)];
        const avgOffsetZ = Math.abs(midPoint.offsetZ) / waveAmplitude;

        const dx = mouseRef.current.x - midPoint.baseX;
        const dy = mouseRef.current.y - midPoint.baseY;
        const distToMouse = Math.sqrt(dx * dx + dy * dy);

        // Smooth gradient: 0 at mouse, 1 far away
        const gradientFactor = Math.min(1, distToMouse / 250);

        // Interpolate between orange (near mouse) and green (far from mouse)
        const r = Math.floor(255 * (1 - gradientFactor) + 74 * gradientFactor);
        const g = Math.floor(122 * (1 - gradientFactor) + 124 * gradientFactor);
        const b = Math.floor(0 * (1 - gradientFactor) + 44 * gradientFactor);

        // 50% more visible (increased from 0.02)
        const baseAlpha = 0.03;
        const waveAlpha = avgOffsetZ * 0.015;
        const mouseAlpha = (1 - gradientFactor) * 0.06;
        const totalAlpha = baseAlpha + waveAlpha + mouseAlpha;

        ctx.strokeStyle = `rgba(${r}, ${g}, ${b}, ${totalAlpha})`;
        ctx.lineWidth = 0.6 + (1 - gradientFactor) * 0.8;
        ctx.stroke();
      }

      // Draw vertical lines with smooth gradient
      for (let j = 0; j < cols; j++) {
        ctx.beginPath();
        for (let i = 0; i < rows; i++) {
          const point = points[i * cols + j];

          if (i === 0) {
            ctx.moveTo(point.x, point.y);
          } else {
            ctx.lineTo(point.x, point.y);
          }
        }

        const midPoint = points[Math.floor(rows / 2) * cols + j];
        const avgOffsetZ = Math.abs(midPoint.offsetZ) / waveAmplitude;

        const dx = mouseRef.current.x - midPoint.baseX;
        const dy = mouseRef.current.y - midPoint.baseY;
        const distToMouse = Math.sqrt(dx * dx + dy * dy);

        // Smooth gradient: 0 at mouse, 1 far away
        const gradientFactor = Math.min(1, distToMouse / 250);

        // Interpolate between orange (near mouse) and green (far from mouse)
        const r = Math.floor(255 * (1 - gradientFactor) + 74 * gradientFactor);
        const g = Math.floor(122 * (1 - gradientFactor) + 124 * gradientFactor);
        const b = Math.floor(0 * (1 - gradientFactor) + 44 * gradientFactor);

        // 50% more visible (increased from 0.02)
        const baseAlpha = 0.03;
        const waveAlpha = avgOffsetZ * 0.015;
        const mouseAlpha = (1 - gradientFactor) * 0.06;
        const totalAlpha = baseAlpha + waveAlpha + mouseAlpha;

        ctx.strokeStyle = `rgba(${r}, ${g}, ${b}, ${totalAlpha})`;
        ctx.lineWidth = 0.6 + (1 - gradientFactor) * 0.8;
        ctx.stroke();
      }

      // Draw dots at intersections with pulse effect
      points.forEach((point) => {
        const dx = mouseRef.current.x - point.baseX;
        const dy = mouseRef.current.y - point.baseY;
        const distance = Math.sqrt(dx * dx + dy * dy);

        // Smooth gradient for color
        const gradientFactor = Math.min(1, distance / 180);
        const r = Math.floor(255 * (1 - gradientFactor) + 74 * gradientFactor);
        const g = Math.floor(122 * (1 - gradientFactor) + 124 * gradientFactor);
        const b = Math.floor(0 * (1 - gradientFactor) + 44 * gradientFactor);

        const proximity = Math.max(0, 1 - distance / 180);
        const waveIntensity = Math.abs(point.offsetZ) / waveAmplitude;

        // Add subtle pulse effect for nearby dots (always active, 2x more visible)
        const pulseBoost = distance < 100 ? pulseEffect * 0.1 : 0;

        if (proximity > 0.05 || waveIntensity > 0.25) {
          const baseSize = 0.5;
          const proximitySize = proximity * 1;
          const waveSize = waveIntensity * 0.6;
          const pulseSize = pulseBoost * 2;
          const size = baseSize + proximitySize + waveSize + pulseSize;

          // 50% more visible (increased from 0.04)
          const baseAlpha = 0.06;
          const proximityAlpha = proximity * 0.09;
          const waveAlpha = waveIntensity * 0.0375;
          const pulseAlpha = pulseBoost * 0.45;
          const alpha = baseAlpha + proximityAlpha + waveAlpha + pulseAlpha;

          ctx.fillStyle = `rgba(${r}, ${g}, ${b}, ${alpha})`;
          ctx.beginPath();
          ctx.arc(point.x, point.y, size, 0, Math.PI * 2);
          ctx.fill();
        }
      });

      animationRef.current = requestAnimationFrame(animate);
    };

    animate();

    // Cleanup
    return () => {
      window.removeEventListener('resize', resize);
      window.removeEventListener('mousemove', handleMouseMove);
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current);
      }
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      className="fixed inset-0 w-full h-full pointer-events-none hidden md:block"
      style={{ zIndex: 0 }}
    />
  );
}
