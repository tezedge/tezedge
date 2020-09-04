OBJDIR = artifacts
TEZOS_BASE_DIR?=src

ifeq ($(shell uname -s),Darwin) # OSX
	LIB_EXTENSION=.dylib
else
	LIB_EXTENSION=.so
endif

all: package

package: build
		ar qs $(OBJDIR)/libtezos.a $(OBJDIR)/*.so

build: build-deps
		cd ${TEZOS_BASE_DIR} && opam config exec -- make
		mkdir -p $(OBJDIR)
		mv ${TEZOS_BASE_DIR}/libtezos-ffi.so $(OBJDIR)/libtezos${LIB_EXTENSION}

build-deps:
		cd ${TEZOS_BASE_DIR} && OPAMYES=1 make build-dev-deps

clean:
		rm -rf artifacts
