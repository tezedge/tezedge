OBJDIR = artifacts

all: package

package: build
		ar qs $(OBJDIR)/libtezos.a $(OBJDIR)/*.o

build: build-deps
		cd src && opam config exec -- make
		mkdir -p $(OBJDIR)
		mv src/libtezos-ffi.so $(OBJDIR)/libtezos.o

build-deps:
		cd src && make build-deps

clean:
		cd src && opam config exec -- make clean
		rm -rf artifacts
