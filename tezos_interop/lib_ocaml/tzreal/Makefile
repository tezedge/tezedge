OBJDIR = artifacts

all: package

package: build
		ar qs $(OBJDIR)/libtzreal.a $(OBJDIR)/*.o

build: build-deps
		cd tezos && opam config exec -- make
		mkdir -p $(OBJDIR)
		mv tezos/libtezos-protocol-ffi.so $(OBJDIR)/libtzreal.o

build-deps:
		cd tezos && make build-deps

clean:
		cd tezos && opam config exec -- make clean
		rm -rf artifacts
