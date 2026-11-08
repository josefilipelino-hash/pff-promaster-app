// Momento Studio — implementação da camada C sobre a LibRaw. Ver momento_libraw.h.
#include "momento_libraw.h"

#include <ctime>
#include <cstring>
#include <new>
#include <cstdio>

#include "libraw/libraw.h"

struct mlr_handle {
  LibRaw *raw;
  bool unpacked;
};

static void copy_str(char *dst, size_t cap, const char *src) {
  if (!src) { dst[0] = 0; return; }
  size_t n = strlen(src);
  if (n >= cap) n = cap - 1;
  memcpy(dst, src, n);
  dst[n] = 0;
}

static void format_time(time_t t, char *out, size_t cap) {
  out[0] = 0;
  if (t <= 0) return;
  struct tm tmv;
#if defined(_WIN32)
  if (localtime_s(&tmv, &t) != 0) return;
#else
  if (!localtime_r(&t, &tmv)) return;
#endif
  // A LibRaw converte a data EXIF com mktime (hora local); localtime devolve a hora de parede original.
  snprintf(out, cap, "%04d-%02d-%02dT%02d:%02d:%02d", tmv.tm_year + 1900, tmv.tm_mon + 1, tmv.tm_mday,
           tmv.tm_hour, tmv.tm_min, tmv.tm_sec);
}

static int public_thumb_format(int internal) {
  switch (internal) {
  case LIBRAW_INTERNAL_THUMBNAIL_JPEG: return LIBRAW_THUMBNAIL_JPEG;
  case LIBRAW_INTERNAL_THUMBNAIL_PPM:
  case LIBRAW_INTERNAL_THUMBNAIL_KODAK_RGB:
  case LIBRAW_INTERNAL_THUMBNAIL_KODAK_YCBCR:
  case LIBRAW_INTERNAL_THUMBNAIL_DNG_YCBCR:
  case LIBRAW_INTERNAL_THUMBNAIL_X3F: return LIBRAW_THUMBNAIL_BITMAP;
  case LIBRAW_INTERNAL_THUMBNAIL_PPM16: return LIBRAW_THUMBNAIL_BITMAP16;
  case LIBRAW_INTERNAL_THUMBNAIL_JPEGXL: return LIBRAW_THUMBNAIL_JPEGXL;
  default: return LIBRAW_THUMBNAIL_UNKNOWN;
  }
}

static void fill_info(LibRaw *r, mlr_info *info) {
  memset(info, 0, sizeof(*info));
  libraw_data_t &d = r->imgdata;
  copy_str(info->make, sizeof info->make, d.idata.make);
  copy_str(info->model, sizeof info->model, d.idata.model);
  copy_str(info->normalized_make, sizeof info->normalized_make, d.idata.normalized_make);
  copy_str(info->normalized_model, sizeof info->normalized_model, d.idata.normalized_model);
  const char *lens = d.lens.Lens[0] ? d.lens.Lens : d.lens.makernotes.Lens;
  copy_str(info->lens, sizeof info->lens, lens);
  copy_str(info->lens_make, sizeof info->lens_make, d.lens.LensMake);
  format_time(d.other.timestamp, info->capture_time, sizeof info->capture_time);
  info->iso = d.other.iso_speed;
  info->shutter = d.other.shutter;
  info->aperture = d.other.aperture;
  info->focal_length = d.other.focal_len;
  info->focal_length_35mm = d.lens.FocalLengthIn35mmFormat;
  info->raw_width = d.sizes.raw_width;
  info->raw_height = d.sizes.raw_height;
  info->width = d.sizes.width;
  info->height = d.sizes.height;
  info->flip = d.sizes.flip;
  info->raw_bps = (int)d.color.raw_bps;
  info->colors = d.idata.colors;
  copy_str(info->cdesc, sizeof info->cdesc, d.idata.cdesc);
  info->dng_version = d.idata.dng_version;
  info->is_foveon = (int)d.idata.is_foveon;
  info->filters_nonzero = d.idata.filters != 0;
  info->decoder_supported = 1;
  libraw_decoder_info_t di;
  memset(&di, 0, sizeof di);
  if (r->get_decoder_info(&di) == LIBRAW_SUCCESS) {
    copy_str(info->decoder_name, sizeof info->decoder_name, di.decoder_name);
    if (di.decoder_flags & (LIBRAW_DECODER_UNSUPPORTED_FORMAT | LIBRAW_DECODER_NOTSET)) info->decoder_supported = 0;
  } else {
    info->decoder_supported = 0;
  }

  // Escolhe a maior preview embutida em formato que sabemos descodificar.
  info->thumb_count = d.thumbs_list.thumbcount;
  info->thumb_index = -1;
  long best_score = -1;
  for (int i = 0; i < d.thumbs_list.thumbcount && i < LIBRAW_THUMBNAIL_MAXCOUNT; i++) {
    const libraw_thumbnail_item_t &t = d.thumbs_list.thumblist[i];
    int fmt = public_thumb_format(t.tformat);
    if (fmt == LIBRAW_THUMBNAIL_UNKNOWN) continue;
    long area = (long)t.twidth * (long)t.theight;
    long score = area > 0 ? area : (long)t.tlength / 3;
    if (fmt == LIBRAW_THUMBNAIL_JPEG) score += 1; // em empate, preferir JPEG
    if (score > best_score) {
      best_score = score;
      info->thumb_index = i;
      info->thumb_format = fmt;
      info->thumb_width = t.twidth;
      info->thumb_height = t.theight;
      info->thumb_length = t.tlength;
      info->thumb_flip = t.tflip;
    }
  }
  if (info->thumb_index < 0 && d.thumbnail.tlength > 0) {
    // Formatos sem lista: usar a miniatura principal.
    info->thumb_index = 0;
    info->thumb_format = d.thumbnail.tformat;
    info->thumb_width = d.thumbnail.twidth;
    info->thumb_height = d.thumbnail.theight;
    info->thumb_length = d.thumbnail.tlength;
    info->thumb_flip = 0xFFFF;
  }
}

static void to_image(libraw_processed_image_t *p, mlr_image *out) {
  out->type = (int)p->type;
  out->width = p->width;
  out->height = p->height;
  out->colors = p->colors;
  out->bits = p->bits;
  out->data = p->data;
  out->size = p->data_size;
  out->opaque = p;
}

extern "C" int mlr_open_buffer(const void *buf, size_t len, unsigned max_mb, mlr_handle **out, mlr_info *info) {
  *out = nullptr;
  mlr_handle *h = nullptr;
  try {
    h = new mlr_handle();
    h->raw = new LibRaw(LIBRAW_OPTIONS_NONE);
    h->unpacked = false;
    if (max_mb > 0) h->raw->imgdata.rawparams.max_raw_memory_mb = max_mb;
    int rc = h->raw->open_buffer(buf, len);
    if (rc != LIBRAW_SUCCESS) {
      mlr_close(h);
      return rc;
    }
    fill_info(h->raw, info);
    *out = h;
    return LIBRAW_SUCCESS;
  } catch (const std::bad_alloc &) {
    mlr_close(h);
    return LIBRAW_UNSUFFICIENT_MEMORY;
  } catch (...) {
    mlr_close(h);
    return MLR_EXCEPTION;
  }
}

extern "C" int mlr_thumb(mlr_handle *h, int index, mlr_image *out) {
  memset(out, 0, sizeof(*out));
  if (!h || !h->raw) return LIBRAW_OUT_OF_ORDER_CALL;
  try {
    int rc = (index > 0 || h->raw->imgdata.thumbs_list.thumbcount > 0) ? h->raw->unpack_thumb_ex(index)
                                                                         : h->raw->unpack_thumb();
    if (rc != LIBRAW_SUCCESS) return rc;
    int err = 0;
    libraw_processed_image_t *p = h->raw->dcraw_make_mem_thumb(&err);
    if (!p) return err != 0 ? err : LIBRAW_NO_THUMBNAIL;
    to_image(p, out);
    return LIBRAW_SUCCESS;
  } catch (const std::bad_alloc &) {
    return LIBRAW_UNSUFFICIENT_MEMORY;
  } catch (...) {
    return MLR_EXCEPTION;
  }
}

extern "C" int mlr_develop(mlr_handle *h, int half_size, mlr_image *out) {
  memset(out, 0, sizeof(*out));
  if (!h || !h->raw) return LIBRAW_OUT_OF_ORDER_CALL;
  try {
    libraw_output_params_t &p = h->raw->imgdata.params;
    p.use_camera_wb = 1;
    p.output_color = 1; // sRGB
    p.output_bps = 8;
    p.half_size = half_size ? 1 : 0;
    p.user_flip = 0;    // sem rotação: a orientação é aplicada pelo pipeline comum (igual aos JPEG)
    p.no_auto_bright = 0;
    if (!h->unpacked) {
      int rc = h->raw->unpack();
      if (rc != LIBRAW_SUCCESS) return rc;
      h->unpacked = true;
    }
    int rc = h->raw->dcraw_process();
    if (rc != LIBRAW_SUCCESS) return rc;
    int err = 0;
    libraw_processed_image_t *img = h->raw->dcraw_make_mem_image(&err);
    if (!img) return err != 0 ? err : LIBRAW_DATA_ERROR;
    to_image(img, out);
    return LIBRAW_SUCCESS;
  } catch (const std::bad_alloc &) {
    return LIBRAW_UNSUFFICIENT_MEMORY;
  } catch (...) {
    return MLR_EXCEPTION;
  }
}

extern "C" void mlr_image_free(mlr_image *img) {
  if (img && img->opaque) {
    LibRaw::dcraw_clear_mem((libraw_processed_image_t *)img->opaque);
    img->opaque = nullptr;
    img->data = nullptr;
    img->size = 0;
  }
}

extern "C" void mlr_close(mlr_handle *h) {
  if (!h) return;
  try {
    delete h->raw;
  } catch (...) {
  }
  delete h;
}

extern "C" const char *mlr_strerror(int code) {
  if (code == MLR_EXCEPTION) return "unexpected C++ exception in LibRaw";
  return libraw_strerror(code);
}

extern "C" const char *mlr_version(void) { return LibRaw::version(); }

extern "C" int mlr_camera_count(void) { return LibRaw::cameraCount(); }
