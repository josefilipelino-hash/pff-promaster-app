/*
 * Momento Studio — camada C mínima sobre a LibRaw (C++).
 *
 * Objetivos:
 *  - expor apenas estruturas POD com layout definido por nós (sem depender do layout
 *    interno de libraw_data_t, que muda entre versões);
 *  - apanhar todas as exceções C++ (a LibRaw lança exceções em erros de memória e de
 *    dados) e devolvê-las como códigos de erro, para que um ficheiro malformado nunca
 *    termine a aplicação através de uma exceção não tratada.
 */
#ifndef MOMENTO_LIBRAW_H
#define MOMENTO_LIBRAW_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Código devolvido quando é apanhada uma exceção C++ inesperada. */
#define MLR_EXCEPTION (-900001)

typedef struct mlr_handle mlr_handle;

typedef struct mlr_info {
  char make[64];
  char model[64];
  char normalized_make[64];
  char normalized_model[64];
  char lens[128];
  char lens_make[128];
  /* Hora local da câmara "AAAA-MM-DDTHH:MM:SS" ou vazio. */
  char capture_time[32];
  float iso;
  float shutter;
  float aperture;
  float focal_length;
  int focal_length_35mm;
  int raw_width;
  int raw_height;
  /* Dimensões de saída antes da rotação. */
  int width;
  int height;
  /* Orientação LibRaw: 0 normal, 3 180°, 5 90° anti-horário, 6 90° horário. */
  int flip;
  int raw_bps;
  int colors;
  char cdesc[8];
  unsigned dng_version;
  int is_foveon;
  int filters_nonzero;
  /* Descodificador escolhido pela LibRaw para os dados do sensor. */
  char decoder_name[64];
  /* 0 se a LibRaw não consegue descodificar os dados do sensor deste ficheiro nesta
   * compilação (ex.: DNG com JPEG XL, NEF High Efficiency) — só a preview embutida. */
  int decoder_supported;
  /* Miniatura/preview embutida escolhida (a maior). */
  int thumb_count;
  int thumb_index;     /* índice para mlr_thumb; -1 = nenhuma */
  int thumb_format;    /* 0 desconhecido, 1 JPEG, 2 bitmap, 3 bitmap16, 7 JPEG XL, outros = não suportado */
  int thumb_width;
  int thumb_height;
  unsigned thumb_length;
  int thumb_flip;
} mlr_info;

typedef struct mlr_image {
  int type;   /* 1 JPEG, 2 bitmap, 3 JPEG XL, 4 H.265 */
  int width;
  int height;
  int colors;
  int bits;
  const uint8_t *data;
  size_t size;
  void *opaque; /* uso interno: libertar com mlr_image_free */
} mlr_image;

/* Abre um ficheiro RAW em memória. O buffer tem de continuar válido até mlr_close. */
int mlr_open_buffer(const void *buf, size_t len, unsigned max_raw_memory_mb, mlr_handle **out, mlr_info *info);

/* Extrai a miniatura/preview embutida com o índice indicado (thumb_index de mlr_info). */
int mlr_thumb(mlr_handle *h, int index, mlr_image *out);

/* Revela o RAW: sRGB 8 bits, balanço de brancos da câmara, SEM rotação (orientação
 * do sensor; a rotação é aplicada pelo chamador com o valor de `flip`).
 * half_size != 0 → metade da resolução (muito mais rápido, sem demosaico completo). */
int mlr_develop(mlr_handle *h, int half_size, mlr_image *out);

/* Revelação para edição: RGB 16 bits LINEAR em Rec. 2020, balanço de brancos da câmara,
 * sem brilho automático, sem rotação. */
int mlr_develop_linear(mlr_handle *h, int half_size, mlr_image *out);

void mlr_image_free(mlr_image *img);
void mlr_close(mlr_handle *h);

const char *mlr_strerror(int code);
const char *mlr_version(void);
int mlr_camera_count(void);

#ifdef __cplusplus
}
#endif

#endif
