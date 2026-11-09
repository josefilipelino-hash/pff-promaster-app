import { imageUrl } from "../api";
import { useStore } from "../store";
import type { FaceRef } from "../types";

/**
 * Rosto recortado da miniatura (CSS, sem pedir nada ao núcleo). As coordenadas do rosto
 * são normalizadas na imagem orientada — a mesma orientação da miniatura.
 */
export function faceCrop(face: FaceRef, photoW: number, photoH: number, size: number) {
  const side = Math.max(face.w * photoW, face.h * photoH) * 1.5;
  const cx = (face.x + face.w / 2) * photoW;
  const cy = (face.y + face.h / 2) * photoH;
  const k = size / Math.max(side, 1);
  return {
    backgroundSize: `${photoW * k}px ${photoH * k}px`,
    backgroundPosition: `${-(cx - side / 2) * k}px ${-(cy - side / 2) * k}px`,
  };
}

export function FaceThumb({ face, size = 56, title }: { face: FaceRef | null; size?: number; title?: string }) {
  const token = useStore((s) => s.project?.imageToken ?? 0);
  const photo = useStore((s) => (face ? s.photos[s.index.get(face.asset) ?? -1] : undefined));
  if (!face || !photo) return <span className="face-thumb empty" style={{ width: size, height: size }} title={title} />;
  return (
    <span
      className="face-thumb"
      title={title}
      style={{ width: size, height: size, backgroundImage: `url("${imageUrl(token, "thumb", face.asset)}")`, ...faceCrop(face, photo.width, photo.height, size) }}
    />
  );
}
